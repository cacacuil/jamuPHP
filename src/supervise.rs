//! Menyalakan Caddy, PHP, dan pekerja; menyalakannya ulang bila mati; menjalankan tugas
//! terjadwal; dan menghentikan semuanya dengan rapi.

use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
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
    /// Pekerja dan tugas terjadwal: dijalankan lewat shell, jadi permintaan berhenti dikirim
    /// ke seluruh grup prosesnya. `queue:work` Laravel menyelesaikan tugas yang sedang jalan.
    Group,
}

/// Mengubah satu baris keluaran sebuah proses menjadi baris log, atau membuangnya.
pub type LogLine = fn(name: &str, line: &str) -> Option<String>;

pub fn plain_log(name: &str, line: &str) -> Option<String> {
    Some(format!("[{name}] {line}"))
}

/// Catatan galat PHP di jalur php-cgi tiba lewat Caddy, terbungkus JSON. Yang ditampilkan
/// isinya saja, seperti di jalur php-fpm.
fn caddy_log(name: &str, line: &str) -> Option<String> {
    if line.contains(r#""msg":"stderr""#)
        && let Ok(v) = serde_json::from_str::<serde_json::Value>(line)
        && let Some(body) = v["body"].as_str()
    {
        let lines: Vec<String> = body.lines().map(|l| format!("[php-cgi] {l}")).collect();
        return Some(lines.join("\n"));
    }
    plain_log(name, line)
}

fn spawn(name: &str, mut cmd: Command, log: LogLine) -> std::io::Result<Child> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    os::prepare(&mut cmd);
    let mut child = cmd.spawn()?;
    let out: Option<Box<dyn Read + Send>> = child.stdout.take().map(|s| Box::new(s) as _);
    let err: Option<Box<dyn Read + Send>> = child.stderr.take().map(|s| Box::new(s) as _);
    for stream in [out, err].into_iter().flatten() {
        let name = name.to_owned();
        thread::spawn(move || {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                if line.trim().is_empty() {
                    continue;
                }
                if let Some(text) = log(&name, &line) {
                    say!("{text}");
                }
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

/// Perintah dari `jamu.toml` atau penjadwal: dijalankan shell di folder project, dengan
/// `php` yang sama dengan yang melayani situs.
fn shell_cmd(plan: &Plan, run: &str) -> Command {
    let mut c = os::shell(run);
    c.current_dir(&plan.dir);
    c.envs(plan.env.iter().map(|(k, v)| (k, v)));
    c
}

struct Service<'a> {
    name: String,
    make: Box<dyn Fn() -> Command + 'a>,
    stop: Stop,
    log: LogLine,
    /// Caddy dan PHP. Situs dianggap siap bila keduanya hidup; pekerja tidak ditunggu.
    core: bool,
    child: Option<Child>,
    started: Instant,
    backoff: Duration,
    retry_at: Instant,
}

impl<'a> Service<'a> {
    fn new(name: &str, make: Box<dyn Fn() -> Command + 'a>, stop: Stop, core: bool) -> Self {
        let now = Instant::now();
        Service {
            name: name.to_owned(),
            make,
            stop,
            log: plain_log,
            core,
            child: None,
            started: now,
            backoff: Duration::from_secs(1),
            retry_at: now,
        }
    }
}

/// Menunggu sebuah proses selesai sampai `deadline`; sesudah itu dimatikan paksa.
fn wait_or_kill(name: &str, child: &mut Child, start: Instant, budget: u64) {
    while child.try_wait().ok().flatten().is_none() {
        if start.elapsed() > Duration::from_secs(budget) {
            say!("jamu: {name} belum selesai setelah {budget} detik; dimatikan paksa");
            os::kill_tree(child);
            let _ = child.wait();
            return;
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn minute_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() / 60)
        .unwrap_or(0)
}

pub fn supervise(plan: &Plan, stop: &AtomicBool, ready: impl FnOnce()) -> Result<(), String> {
    let (php_name, sock) = match &plan.php {
        Php::Fpm { sock, .. } => ("php-fpm", Some(sock)),
        Php::Cgi { .. } => ("php-cgi", None),
    };
    // PHP lebih dulu, supaya sudah mendengarkan saat Caddy mulai menerima permintaan.
    let mut services = vec![
        Service::new(php_name, Box::new(|| php_cmd(plan)), Stop::Quit, true),
        Service::new("caddy", Box::new(|| caddy_cmd(plan)), Stop::Term, true),
    ];
    for w in &plan.workers {
        services.push(Service::new(
            &w.name,
            Box::new(|| shell_cmd(plan, &w.run)),
            Stop::Group,
            false,
        ));
    }
    services[1].log = caddy_log;
    let mut ready = Some(ready);
    // Tugas terjadwal yang sedang berjalan. Seperti cron: dijalankan tiap pergantian menit,
    // tanpa menunggu yang sebelumnya selesai.
    let mut runs: Vec<Child> = Vec::new();
    let mut last_minute = minute_now();

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
                None if Instant::now() >= s.retry_at => match spawn(&s.name, (s.make)(), s.log) {
                    Ok(child) => {
                        s.child = Some(child);
                        s.started = Instant::now();
                    }
                    Err(e) if s.core => return Err(format!("gagal menyalakan {}: {e}", s.name)),
                    // Pekerja yang tidak bisa dinyalakan tidak boleh menjatuhkan situsnya.
                    Err(e) => {
                        say!(
                            "jamu: pekerja {} gagal dinyalakan ({e}); dicoba lagi dalam {} detik",
                            s.name,
                            s.backoff.as_secs()
                        );
                        s.retry_at = Instant::now() + s.backoff;
                        s.backoff = (s.backoff * 2).min(Duration::from_secs(30));
                    }
                },
                None => {}
            }
        }
        if let Some(task) = &plan.schedule {
            let minute = minute_now();
            if minute != last_minute {
                last_minute = minute;
                match spawn(&task.name, shell_cmd(plan, &task.run), task.log) {
                    Ok(child) => runs.push(child),
                    Err(e) => say!("jamu: {} gagal dijalankan: {e}", task.name),
                }
            }
            runs.retain_mut(|child| match child.try_wait() {
                Ok(Some(status)) => {
                    if !status.success() {
                        say!("jamu: {} berakhir dengan galat ({status})", task.name);
                    }
                    false
                }
                Ok(None) => true,
                Err(_) => false,
            });
        }
        os::reap_orphans(
            &services
                .iter()
                .filter_map(|s| s.child.as_ref())
                .chain(runs.iter())
                .map(Child::id)
                .collect::<Vec<_>>(),
        );
        if services
            .iter()
            .filter(|s| s.core)
            .all(|s| s.child.is_some())
            && let Some(f) = ready.take()
        {
            f();
        }
        thread::sleep(Duration::from_millis(200));
    }

    say!("jamu: berhenti...");
    let start = Instant::now();
    // Pekerja dan tugas terjadwal diberi tahu lebih dulu, lalu ditunggu paling akhir: mereka
    // mendapat waktu terpanjang untuk menyelesaikan tugas yang sedang berjalan.
    let (core, workers): (Vec<_>, Vec<_>) = services.iter_mut().partition(|s| s.core);
    let mut background: Vec<(&str, &mut Child)> = workers
        .into_iter()
        .filter_map(|s| s.child.as_mut().map(|c| (s.name.as_str(), c)))
        .chain(
            runs.iter_mut()
                .map(|c| (plan.schedule.as_ref().map_or("schedule", |t| &t.name), c)),
        )
        .collect();
    for (_, child) in background.iter_mut() {
        os::ask_to_stop(child, Stop::Group);
    }
    // Berurutan, Caddy dulu: ia berhenti menerima koneksi baru dan menunggu permintaan yang
    // sedang berjalan, sementara PHP masih hidup untuk menyelesaikannya. Baru setelah itu
    // PHP dihentikan. Total 9 detik, di bawah batas 10 detik bawaan `docker stop`.
    for (s, budget) in core.into_iter().rev().zip([8, 9]) {
        let Some(child) = &mut s.child else { continue };
        os::ask_to_stop(child, s.stop);
        wait_or_kill(&s.name, child, start, budget);
    }
    for (name, child) in background {
        wait_or_kill(name, child, start, 9);
    }
    if let Some(sock) = sock {
        let _ = fs::remove_file(sock);
    }
    Ok(())
}
