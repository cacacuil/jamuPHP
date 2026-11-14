//! Menyalakan Caddy dan PHP, menyalakannya ulang bila mati, dan menghentikannya dengan rapi.

use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use std::{fs, thread};

use crate::config::{Php, Plan, cgi_workers};
use crate::os;

/// Cara meminta sebuah proses berhenti dengan rapi.
#[derive(Clone, Copy)]
pub enum Stop {
    /// Caddy: selesaikan permintaan yang sedang berjalan, lalu keluar.
    Term,
    /// PHP: SIGQUIT di Linux, dimatikan di Windows.
    Quit,
}

fn spawn(name: &'static str, mut cmd: Command) -> std::io::Result<Child> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    os::prepare(&mut cmd);
    let mut child = cmd.spawn()?;
    let out: Option<Box<dyn Read + Send>> = child.stdout.take().map(|s| Box::new(s) as _);
    let err: Option<Box<dyn Read + Send>> = child.stderr.take().map(|s| Box::new(s) as _);
    for stream in [out, err].into_iter().flatten() {
        thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                say!("[{name}] {line}");
            }
        });
    }
    Ok(child)
}

fn caddy_cmd(plan: &Plan) -> Command {
    let mut c = Command::new(&plan.caddy);
    c.args(["run", "--adapter", "caddyfile", "--config"])
        .arg(&plan.caddyfile);
    // Caddy menyimpan catatan kecil di folder data dan config pengguna. Arahkan ke folder
    // state supaya tidak menyentuh HOME, yang di container sering tidak bisa ditulis.
    if let Some(state) = plan.caddyfile.parent() {
        c.env("XDG_DATA_HOME", state.join("caddy-data"))
            .env("XDG_CONFIG_HOME", state.join("caddy-config"));
    }
    c
}

fn php_cmd(plan: &Plan) -> Command {
    match &plan.php {
        Php::Fpm { bin, conf, .. } => {
            let mut c = Command::new(bin);
            c.args(["--nodaemonize", "--fpm-config"]).arg(conf);
            if os::is_root() {
                c.arg("--allow-to-run-as-root");
            }
            c
        }
        Php::Cgi { bin, ini, port } => {
            let mut c = Command::new(bin);
            c.arg("-b")
                .arg(format!("127.0.0.1:{port}"))
                .arg("-c")
                .arg(ini);
            // Pekerja diganti setelah 500 permintaan, supaya kebocoran memori tidak menumpuk.
            c.env("PHP_FCGI_CHILDREN", cgi_workers().to_string())
                .env("PHP_FCGI_MAX_REQUESTS", "500");
            c
        }
    }
}

struct Service {
    name: &'static str,
    make: fn(&Plan) -> Command,
    stop: Stop,
    child: Option<Child>,
    started: Instant,
    backoff: Duration,
    retry_at: Instant,
}

pub fn supervise(plan: &Plan, stop: &AtomicBool, ready: impl FnOnce()) -> Result<(), String> {
    let now = Instant::now();
    let new = |name, make, stop| Service {
        name,
        make,
        stop,
        child: None,
        started: now,
        backoff: Duration::from_secs(1),
        retry_at: now,
    };
    let (php_name, sock) = match &plan.php {
        Php::Fpm { sock, .. } => ("php-fpm", Some(sock)),
        Php::Cgi { .. } => ("php-cgi", None),
    };
    // PHP lebih dulu, supaya sudah mendengarkan saat Caddy mulai menerima permintaan.
    let mut services = [
        new(php_name, php_cmd as fn(&Plan) -> Command, Stop::Quit),
        new("caddy", caddy_cmd, Stop::Term),
    ];
    let mut ready = Some(ready);

    while !stop.load(Ordering::SeqCst) {
        for s in services.iter_mut() {
            match &mut s.child {
                Some(child) => {
                    if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
                        // Yang bertahan lama dianggap sehat; yang langsung mati diberi jeda bertambah.
                        if s.started.elapsed() > Duration::from_secs(60) {
                            s.backoff = Duration::from_secs(1);
                        }
                        say!(
                            "jamu: {} berhenti ({status}); dinyalakan ulang dalam {} detik",
                            s.name,
                            s.backoff.as_secs()
                        );
                        // Anak yang tertinggal, misalnya pekerja PHP yang induknya mati, masih
                        // memegang soket. Bersihkan semuanya sebelum menyalakan ulang.
                        os::kill_tree(child);
                        if let (Some(sock), true) = (sock, s.name == php_name) {
                            let _ = fs::remove_file(sock);
                        }
                        s.child = None;
                        s.retry_at = Instant::now() + s.backoff;
                        s.backoff = (s.backoff * 2).min(Duration::from_secs(30));
                    }
                }
                None if Instant::now() >= s.retry_at => {
                    let child = spawn(s.name, (s.make)(plan))
                        .map_err(|e| format!("gagal menyalakan {}: {e}", s.name))?;
                    s.child = Some(child);
                    s.started = Instant::now();
                }
                None => {}
            }
        }
        os::reap_orphans(
            &services
                .iter()
                .filter_map(|s| s.child.as_ref().map(Child::id))
                .collect::<Vec<_>>(),
        );
        if services.iter().all(|s| s.child.is_some()) {
            if let Some(f) = ready.take() {
                f();
            }
        }
        thread::sleep(Duration::from_millis(200));
    }

    say!("jamu: berhenti...");
    // Berurutan, Caddy dulu: ia berhenti menerima koneksi baru dan menunggu permintaan yang
    // sedang berjalan, sementara PHP masih hidup untuk menyelesaikannya. Baru setelah itu
    // PHP dihentikan. Total 9 detik, di bawah batas 10 detik bawaan `docker stop`.
    let start = Instant::now();
    for (s, budget) in services.iter_mut().rev().zip([8, 9]) {
        let Some(child) = &mut s.child else { continue };
        os::ask_to_stop(child, s.stop);
        while child.try_wait().ok().flatten().is_none() {
            if start.elapsed() > Duration::from_secs(budget) {
                say!(
                    "jamu: {} belum selesai setelah {budget} detik; dimatikan paksa",
                    s.name
                );
                os::kill_tree(child);
                let _ = child.wait();
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
    }
    if let Some(sock) = sock {
        let _ = fs::remove_file(sock);
    }
    Ok(())
}
