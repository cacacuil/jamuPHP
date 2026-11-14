//! Perintah `serve` dan `health`: memilih port, menyiapkan folder state, menulis
//! konfigurasi, lalu menyerahkan prosesnya ke supervisor.

use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use std::{env, fs, thread};

use crate::config::{PING_PATH, Php, Plan, caddyfile, cgi_ini, cgi_workers, fpm_conf, workers};
use crate::detect::{Kind, Project, detect, kind_name};
use crate::supervise::supervise;
use crate::{Opts, USAGE, os};

fn find_bin(env_name: &str, names: &[&str]) -> Result<PathBuf, String> {
    if let Some(p) = env::var_os(env_name).filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(p));
    }
    // Yang dibawa bersama jamu (di folder yang sama, atau subfolder `php`) menang atas PATH.
    let beside = env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf));
    let bundled = beside.iter().flat_map(|d| [d.clone(), d.join("php")]);
    let path = env::var_os("PATH").unwrap_or_default();
    for dir in bundled
        .chain(env::split_paths(&path))
        .chain(os::BIN_DIRS.iter().map(PathBuf::from))
    {
        for name in names {
            let p = dir.join(format!("{name}{}", env::consts::EXE_SUFFIX));
            if p.is_file() {
                return Ok(p);
            }
        }
    }
    Err(format!(
        "{} tidak ditemukan. Pasang, atau tunjuk lewat variabel {env_name}.",
        names[0]
    ))
}

fn fnv1a(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c9dc5u32, |h, b| {
        (h ^ *b as u32).wrapping_mul(0x01000193)
    })
}

fn state_dir(base: &Path, project: &Path) -> PathBuf {
    let name: String = project
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let hash = fnv1a(project.as_os_str().as_encoded_bytes());
    base.join(format!("{name}-{hash:08x}"))
}

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

/// Folder yang harus bisa ditulis PHP. Di container ini sumber masalah nomor satu: volume
/// milik root, PHP berjalan sebagai www-data, lalu unggahan gagal tanpa pesan yang jelas.
fn warn_unwritable(p: &Project) {
    let dirs = match p.kind {
        Kind::WordPress => vec![p.root.join("wp-content")],
        Kind::Laravel => vec![p.dir.join("storage"), p.dir.join("bootstrap/cache")],
        Kind::Php => vec![],
    };
    for d in dirs.iter().filter(|d| d.is_dir()) {
        if let Some(who) = os::unwritable(d) {
            say!(
                "jamu: peringatan: {} tidak bisa ditulis oleh {who}; unggahan, cache, atau log situs akan gagal",
                d.display()
            );
        }
    }
}

pub fn serve(o: Opts) -> Result<(), String> {
    os::init();
    let project = detect(&o.dir);
    let dev = o.domain.is_none() && !o.behind_proxy;
    let caddy = find_bin("JAMU_CADDY", &["caddy"])?;
    // JAMU_PHP_CGI di Linux memaksa jalur php-cgi, supaya jalur Windows bisa diuji di Linux.
    let cgi = cfg!(windows) || env::var_os("JAMU_PHP_CGI").is_some();
    let php_bin = if cgi {
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

    let free = |p: u16| port_free(p, dev);
    let port = match (&o.domain, o.port) {
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

    let mut state = state_dir(&os::state_base(), &project.dir);
    if let Err(first) = fs::create_dir_all(state.join("caddy")) {
        let wanted = state.clone();
        let fallback = os::fallback_state_base().map(|base| state_dir(&base, &project.dir));
        match fallback.filter(|s| fs::create_dir_all(s.join("caddy")).is_ok()) {
            Some(s) => state = s,
            None => {
                return Err(format!(
                    "Tidak ada folder yang bisa ditulis untuk konfigurasi: {} ({first}), dan folder sementara juga tidak bisa. \
                     Tunjuk folder yang bisa ditulis lewat variabel XDG_STATE_HOME, atau pasang tmpfs di sana.",
                    wanted.display()
                ));
            }
        }
    }
    let write = |p: &Path, text: String| {
        fs::write(p, text).map_err(|e| format!("tidak bisa menulis {}: {e}", p.display()))
    };
    let php = if cgi {
        let ini = state.join("php.ini");
        write(&ini, cgi_ini(&php_bin, dev))?;
        Php::Cgi {
            bin: php_bin,
            ini,
            port: free_port()?,
        }
    } else {
        // Panjang path soket Unix dibatasi kernel (108 byte).
        let mut sock = state.join("php-fpm.sock");
        if sock.as_os_str().len() > 100 {
            sock = PathBuf::from(format!(
                "/tmp/jamu-{:08x}.sock",
                fnv1a(state.as_os_str().as_encoded_bytes())
            ));
        }
        let _ = fs::remove_file(&sock);
        let conf = state.join("php-fpm.conf");
        write(&conf, fpm_conf(&sock, dev))?;
        Php::Fpm {
            bin: php_bin,
            conf,
            sock,
        }
    };
    let plan = Plan {
        caddy,
        caddyfile: state.join("Caddyfile"),
        php,
    };
    write(
        &plan.caddyfile,
        caddyfile(&project, &o, port, &state, &plan.php),
    )?;

    let (php_bin, how) = match &plan.php {
        Php::Fpm { bin, .. } => (
            bin,
            format!("php-fpm ondemand, maksimal {} pekerja", workers()),
        ),
        Php::Cgi { bin, .. } => (bin, format!("php-cgi, {} pekerja", cgi_workers())),
    };
    let php_version = Command::new(php_bin)
        .arg("-v")
        .output()
        .ok()
        .and_then(|out| {
            String::from_utf8_lossy(&out.stdout)
                .split_whitespace()
                .nth(1)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "?".into());
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
    warn_unwritable(&project);

    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    ctrlc::set_handler(move || flag.store(true, Ordering::SeqCst)).map_err(|e| e.to_string())?;

    let url = match &o.domain {
        Some(d) => format!("https://{d}"),
        None if o.behind_proxy => format!("port {port}, di belakang proxy"),
        None => format!("http://localhost:{port}"),
    };
    let domain_mode = o.domain.is_some();
    supervise(&plan, &stop, move || {
        // Tunggu sampai port benar-benar menerima koneksi sebelum mengumumkan siap.
        thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(15);
            while Instant::now() < until {
                if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                    let note = if domain_mode {
                        " (sertifikat diterbitkan dan diperbarui otomatis)"
                    } else {
                        ""
                    };
                    say!("jamu: siap di {url}{note}");
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
