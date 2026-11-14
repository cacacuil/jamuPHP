//! Perintah `serve` dan `health`: memilih port, menyiapkan folder state, menulis
//! konfigurasi, lalu menyerahkan prosesnya ke supervisor.

use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use std::{env, fs, thread};

use crate::cli::{check_version, php_cli, php_version};
use crate::config::{
    PING_PATH, Php, Plan, Task, caddyfile, cgi_ini, cgi_workers, fpm_conf, workers,
};
use crate::detect::{Kind, Project, kind_name};
use crate::mail;
use crate::paths::{find_bin, fnv1a, open_state};
use crate::settings;
use crate::supervise::{plain_log, supervise};
use crate::{Opts, USAGE, os};

/// Port dianggap bebas bila tidak ada yang menjawab di loopback dan kita bisa mengikatnya.
/// Mode dev hanya menguji loopback: mengikat 0.0.0.0 di Windows memunculkan dialog firewall.
fn port_free(port: u16, loopback_only: bool) -> bool {
    let local = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    if TcpStream::connect_timeout(&local, Duration::from_millis(200)).is_ok() {
        return false;
    }
    TcpListener::bind((
        if loopback_only {
            "127.0.0.1"
        } else {
            "0.0.0.0"
        },
        port,
    ))
    .is_ok()
}

/// Port loopback bebas untuk php-cgi.
fn free_port() -> Result<u16, String> {
    let l = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| e.to_string())?;
    l.local_addr().map(|a| a.port()).map_err(|e| e.to_string())
}

/// Folder yang harus bisa ditulis PHP tetapi tidak bisa, beserta siapa yang gagal menulis.
/// Di container ini sumber masalah nomor satu: volume milik root, PHP berjalan sebagai
/// www-data, lalu unggahan gagal tanpa pesan yang jelas.
pub fn unwritable(p: &Project) -> Vec<(PathBuf, String)> {
    let dirs = match p.kind {
        Kind::WordPress => vec![p.root.join("wp-content")],
        Kind::Laravel => vec![p.dir.join("storage"), p.dir.join("bootstrap/cache")],
        Kind::Php => vec![],
    };
    dirs.into_iter()
        .filter(|d| d.is_dir())
        .filter_map(|d| os::unwritable(&d).map(|who| (d, who)))
        .collect()
}

/// Program PHP yang melayani situs: php-fpm, atau php-cgi di Windows.
/// JAMU_PHP_CGI di Linux memaksa jalur php-cgi, supaya jalur Windows bisa diuji di Linux.
pub fn find_php() -> Result<(bool, PathBuf), String> {
    let cgi = cfg!(windows) || env::var_os("JAMU_PHP_CGI").is_some();
    let bin = if cgi {
        find_bin(
            "JAMU_PHP_CGI",
            &[
                "php-cgi",
                "php-cgi8.5",
                "php-cgi8.4",
                "php-cgi8.3",
                "php-cgi8.2",
            ],
        )?
    } else {
        find_bin(
            "JAMU_PHP_FPM",
            &[
                "php-fpm",
                "php-fpm8.5",
                "php-fpm8.4",
                "php-fpm8.3",
                "php-fpm8.2",
            ],
        )?
    };
    Ok((cgi, bin))
}

pub fn write(p: &Path, text: String) -> Result<(), String> {
    fs::write(p, text).map_err(|e| format!("tidak bisa menulis {}: {e}", p.display()))
}

/// Menulis konfigurasi PHP ke `dir`. Soket php-fpm selalu di folder state, supaya `check`
/// menguji konfigurasi yang sama dengan yang dipakai `serve`.
pub fn write_php(
    dir: &Path,
    state: &Path,
    (cgi, bin): (bool, PathBuf),
    dev: bool,
    user: &[(String, String)],
) -> Result<Php, String> {
    if cgi {
        let ini = dir.join("php.ini");
        write(&ini, cgi_ini(&bin, dev, user))?;
        return Ok(Php::Cgi {
            bin,
            ini,
            port: free_port()?,
        });
    }
    // Panjang path soket Unix dibatasi kernel (108 byte).
    let mut sock = state.join("php-fpm.sock");
    if sock.as_os_str().len() > 100 {
        sock = PathBuf::from(format!(
            "/tmp/jamu-{:08x}.sock",
            fnv1a(state.as_os_str().as_encoded_bytes())
        ));
    }
    let conf = dir.join("php-fpm.conf");
    write(&conf, fpm_conf(&sock, dev, user))?;
    Ok(Php::Fpm { bin, conf, sock })
}

pub fn serve(o: Opts) -> Result<(), String> {
    os::init();
    let (project, settings) = settings::load(&o.dir)?;
    let dev = o.domains.is_empty() && !o.behind_proxy;
    let caddy = find_bin("JAMU_CADDY", &["caddy"])?;
    let found_php = find_php()?;

    let free = |p: u16| port_free(p, dev);
    let port = match (o.domains.first(), o.port) {
        (Some(_), _) => {
            for p in [80, 443] {
                if !free(p) {
                    return Err(format!(
                        "Port {p} sedang dipakai{}. Mode domain butuh port 80 dan 443.",
                        os::port_holder(p)
                    ));
                }
            }
            443
        }
        (None, None) if o.behind_proxy && free(8080) => 8080,
        (None, None) if o.behind_proxy => {
            return Err(format!(
                "Port 8080 sedang dipakai{}.",
                os::port_holder(8080)
            ));
        }
        (None, Some(p)) if free(p) => p,
        (None, Some(p)) => return Err(format!("Port {p} sedang dipakai{}.", os::port_holder(p))),
        (None, None) => {
            let p = (8080..8180)
                .find(|p| free(*p))
                .ok_or("Tidak ada port bebas di 8080-8179.")?;
            if p != 8080 {
                say!(
                    "jamu: port 8080 sedang dipakai{}; memakai {p}",
                    os::port_holder(8080)
                );
            }
            p
        }
    };

    let state = open_state(&project.dir)?;
    let mut ini = settings.ini();
    mail::add_sendmail(&mut ini, &state)?;
    let php = write_php(&state, &state, found_php, dev, &ini)?;
    if let Php::Fpm { sock, .. } = &php {
        let _ = fs::remove_file(sock);
    }
    let (php_bin, how) = match &php {
        Php::Fpm { bin, .. } => (
            bin,
            format!("php-fpm ondemand, maksimal {} pekerja", workers()),
        ),
        Php::Cgi { bin, .. } => (bin, format!("php-cgi, {} pekerja", cgi_workers())),
    };
    let php_version = php_version(php_bin);
    check_version(&settings, &php_version)?;

    let run_workers: Vec<Task> = settings
        .workers
        .iter()
        .map(|w| Task {
            name: w.name.clone(),
            run: w.run.clone(),
            log: plain_log,
            overlap: false,
        })
        .collect();
    // Penjadwal menggantikan baris cron sistem. Di mode dev ia diam kecuali diminta: tugas
    // terjadwal tidak boleh jalan sendiri di laptop.
    let wanted = settings.scheduler.unwrap_or(!dev);
    let wp_cron = project.dir.join("wp-cron.php").is_file();
    let schedule = match project.kind {
        // Pengganti `* * * * * php artisan schedule:run`.
        Kind::Laravel if wanted => Some(Task {
            name: "schedule".into(),
            run: "php artisan schedule:run".into(),
            // Tiap menit Laravel melapor bahwa tidak ada yang perlu dijalankan; itu bukan berita.
            log: |name, line| {
                (!line.contains("No scheduled commands are ready to run"))
                    .then(|| format!("[{name}] {line}"))
            },
            overlap: true,
        }),
        // Tanpa ini wp-cron hanya jalan saat ada pengunjung: tulisan terjadwal di situs sepi
        // terlambat terbit, dan berkas cache yang kedaluwarsa tidak dibuang. Menjalankan
        // dua sekaligus tidak ada gunanya, jadi yang baru menunggu yang lama selesai.
        Kind::WordPress if wanted && wp_cron => Some(Task {
            name: "schedule".into(),
            run: "php wp-cron.php".into(),
            log: plain_log,
            overlap: false,
        }),
        _ => {
            if settings.scheduler == Some(true) {
                say!(
                    "jamu: scheduler = true diabaikan: penjadwal hanya untuk Laravel dan WordPress"
                );
            }
            None
        }
    };
    // PHP baris perintah hanya dicari bila ada yang membutuhkannya.
    let env = if run_workers.is_empty() && schedule.is_none() {
        Vec::new()
    } else {
        php_cli(&project.dir, &settings)?.env
    };
    let plan = Plan {
        caddy,
        caddyfile: state.join("Caddyfile"),
        php,
        dir: project.dir.clone(),
        env,
        workers: run_workers,
        schedule,
    };
    write(
        &plan.caddyfile,
        caddyfile(&project, &o, port, &state, &plan.php),
    )?;

    let rel_root = project
        .root
        .strip_prefix(&project.dir)
        .ok()
        .filter(|r| !r.as_os_str().is_empty());
    say!(
        "jamu: {} di {} (document root: {})",
        kind_name(project.kind),
        project.dir.display(),
        rel_root
            .map(|r| r.display().to_string())
            .unwrap_or_else(|| ".".into())
    );
    say!(
        "jamu: PHP {php_version}{}, {how}",
        project
            .php_wanted
            .as_ref()
            .map(|w| format!(" (composer.json meminta {w})"))
            .unwrap_or_default()
    );
    say!("jamu: konfigurasi di {}", state.display());
    for w in &plan.workers {
        say!("jamu: pekerja {}: {}", w.name, w.run);
    }
    if let Some(task) = &plan.schedule {
        say!("jamu: penjadwal aktif ({} tiap menit)", task.run);
    }
    if let Some(relay) = mail::relay()? {
        say!("jamu: mail() diteruskan ke {}:{}", relay.host, relay.port);
    }
    for (d, who) in unwritable(&project) {
        say!(
            "jamu: peringatan: {} tidak bisa ditulis oleh {who}; unggahan, cache, atau log situs akan gagal",
            d.display()
        );
    }

    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    ctrlc::set_handler(move || flag.store(true, Ordering::SeqCst)).map_err(|e| e.to_string())?;

    let url = match o.domains.first() {
        Some(d) => format!("https://{d}"),
        None if o.behind_proxy => format!("port {port}, di belakang proxy"),
        None => format!("http://localhost:{port}"),
    };
    supervise(&plan, &stop, move || {
        // Tunggu sampai port benar-benar menerima koneksi sebelum mengumumkan siap.
        thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(15);
            while Instant::now() < until {
                if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                    say!("jamu: siap di {url}");
                    return;
                }
                thread::sleep(Duration::from_millis(100));
            }
            say!("jamu: port {port} belum menerima koneksi setelah 15 detik; periksa log di atas");
        });
    })
}

/// Untuk HEALTHCHECK container: keluar 0 hanya bila Caddy dan php-fpm sama-sama menjawab.
pub fn health(args: &[String]) -> Result<(), String> {
    let port: u16 = match args {
        [] => 8080,
        [flag, n] if flag == "--port" => n.parse().map_err(|_| "--port harus angka 1-65535")?,
        _ => return Err(USAGE.into()),
    };
    let fail = |e: std::io::Error| format!("port {port} tidak menjawab: {e}");
    let mut conn =
        TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), Duration::from_secs(2))
            .map_err(fail)?;
    conn.set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(fail)?;
    std::io::Write::write_all(
        &mut conn,
        format!("GET {PING_PATH} HTTP/1.0\r\nHost: localhost\r\n\r\n").as_bytes(),
    )
    .map_err(fail)?;
    let mut reply = String::new();
    conn.read_to_string(&mut reply).map_err(fail)?;
    if reply.split_whitespace().nth(1) == Some("200") && reply.trim_end().ends_with("pong") {
        return Ok(());
    }
    Err(format!(
        "php-fpm tidak menjawab ping: {}",
        reply.lines().next().unwrap_or("jawaban kosong")
    ))
}
