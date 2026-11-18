//! Menyalakan Caddy, PHP, dan pekerja; menyalakannya ulang bila mati; menjalankan tugas
//! terjadwal; dan menghentikan semuanya dengan rapi.

use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
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

/// Log akses Caddy, diringkas menjadi satu baris: alamat pengunjung, permintaan, kode
/// jawaban, dan lama dilayani.
///
/// Baris ini datang sekali per permintaan, jadi hanya bagian yang dipakai yang diuraikan;
/// header dan sisanya dilewati tanpa disalin.
fn access_line(line: &str) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct Access {
        request: Request,
        status: u64,
        duration: f64,
    }
    #[derive(serde::Deserialize)]
    struct Request {
        client_ip: String,
        method: String,
        uri: String,
    }
    if !line.contains(r#""logger":"http.log.access"#) {
        return None;
    }
    let a: Access = serde_json::from_str(line).ok()?;
    Some(format!(
        "[akses] {} {} {} {} {} ms",
        a.request.client_ip,
        a.request.method,
        a.request.uri,
        a.status,
        (a.duration * 1000.0).round()
    ))
}

/// Kabar soal sertifikat, dalam satu baris yang bisa dibaca tanpa mengenal ACME.
fn cert_line(v: &serde_json::Value) -> Option<String> {
    let id = v["identifier"].as_str();
    match v["msg"].as_str()? {
        "certificate obtained successfully" => {
            Some(format!("jamu: sertifikat untuk {} terbit", id?))
        }
        "certificate renewed successfully" => {
            Some(format!("jamu: sertifikat untuk {} diperpanjang", id?))
        }
        "could not get certificate from issuer" => Some(format!(
            "jamu: sertifikat untuk {} belum terbit: {}",
            id?,
            v["error"].as_str()?
        )),
        "will retry" if v["logger"].as_str()?.starts_with("tls.") => {
            let secs = v["retrying_in"].as_f64()?;
            let wait = if secs >= 3600.0 {
                format!("{} jam", (secs / 3600.0).round())
            } else {
                format!("{} menit", (secs / 60.0).ceil())
            };
            Some(format!(
                "jamu: sertifikat dicoba lagi dalam {wait}; situs tetap dilayani"
            ))
        }
        // Penolakan dari CA: ia yang melihat domain ini dari internet, jadi penjelasannya
        // diteruskan apa adanya, didahului artinya.
        "challenge failed" => {
            let problem = &v["problem"];
            let why = match problem["type"].as_str()?.rsplit(':').next()? {
                "dns" => "DNS domain itu belum benar",
                "connection" => "alamat di DNS domain itu tidak sampai ke server ini",
                "unauthorized" => "yang menjawab di alamat DNS domain itu bukan server ini",
                "caa" => "rekaman CAA domain itu melarang CA ini",
                "rateLimited" => "batas permintaan CA tercapai",
                _ => "ditolak CA",
            };
            Some(format!(
                "jamu: pemeriksaan domain {} gagal: {why} ({})",
                id?,
                problem["detail"].as_str()?
            ))
        }
        _ => None,
    }
}

/// Peringatan Caddy yang selalu muncul karena cara jamu memakainya, bukan karena ada masalah.
const CADDY_NOISE: &[&str] = &[
    "admin endpoint disabled",
    "HTTP/2 skipped because it requires TLS",
    "HTTP/3 skipped because it requires TLS",
];

/// Baris Caddy yang ditulis ulang: log akses, kabar sertifikat, dan catatan galat PHP di
/// jalur php-cgi, yang tiba lewat Caddy terbungkus JSON dan ditampilkan isinya saja seperti
/// di jalur php-fpm. Sisanya diteruskan apa adanya.
fn caddy_log(name: &str, line: &str) -> Option<String> {
    if let Some(text) = access_line(line) {
        return Some(text);
    }
    let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
        return plain_log(name, line);
    };
    let msg = v["msg"].as_str().unwrap_or_default();
    if msg == "stderr"
        && let Some(body) = v["body"].as_str()
    {
        let lines: Vec<String> = body.lines().map(|l| format!("[php-cgi] {l}")).collect();
        return Some(lines.join("\n"));
    }
    if let Some(text) = cert_line(&v) {
        return Some(text);
    }
    // Selain log akses, baris INFO hanya datang dari pencatat sertifikat: langkah-langkah
    // kecil yang tidak perlu dibaca siapa pun.
    if v["level"] == "info" || CADDY_NOISE.contains(&msg) {
        return None;
    }
    plain_log(name, line)
}

/// Tiga kabar php-fpm yang menjelaskan kenapa situs lambat atau permintaan gagal, ditulis
/// ulang supaya bisa dibaca tanpa mengenal php-fpm. Sisanya diteruskan apa adanya.
fn fpm_log(name: &str, line: &str) -> Option<String> {
    if line.contains("execution timed out")
        && let Some(request) = line.split("(request: \"").nth(1)
        && let Some(request) = request.split('"').next()
    {
        return Some(format!(
            "jamu: permintaan {request} dihentikan: melewati batas waktu (timeout di jamu.toml)"
        ));
    }
    if line.contains("reached max_children setting") || line.contains("reached pm.max_children") {
        return Some(
            "jamu: semua pekerja PHP sedang sibuk; permintaan berikutnya menunggu giliran".into(),
        );
    }
    if line.contains("exited on signal 9 (SIGKILL)") {
        return Some(format!(
            "[{name}] {line}\njamu: pekerja PHP dimatikan paksa oleh sistem; biasanya karena memori container habis"
        ));
    }
    // Pekerja yang selesai gilirannya lalu diganti adalah kejadian rutin, bukan berita.
    if line.contains("NOTICE: [pool")
        && (line.ends_with(" started") || line.contains("exited with code 0 after"))
    {
        return None;
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

/// Akhir sebuah perintah yang dijalankan sekali dan ditunggu.
#[derive(Debug)]
pub enum Once {
    Done,
    Failed(ExitStatus),
    TimedOut,
    /// jamu diminta berhenti selagi perintahnya berjalan.
    Stopped,
}

/// Menjalankan satu perintah sampai selesai, sebelum situs dinyalakan. Keluarannya ditulis
/// ke berkas `out`. Perintah beserta anak-anaknya dimatikan bila melewati `budget`, atau
/// bila jamu diminta berhenti: langkah persiapan tidak boleh menyandera start maupun stop.
pub fn run_once(
    mut cmd: Command,
    out: &Path,
    budget: Duration,
    stop: &AtomicBool,
) -> std::io::Result<Once> {
    let file = fs::File::create(out)?;
    cmd.stdin(Stdio::null())
        .stdout(file.try_clone()?)
        .stderr(file);
    os::prepare(&mut cmd);
    let mut child = cmd.spawn()?;
    let start = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(if status.success() {
                Once::Done
            } else {
                Once::Failed(status)
            });
        }
        let stopped = stop.load(Ordering::SeqCst);
        if stopped || start.elapsed() > budget {
            os::kill_tree(&mut child);
            let _ = child.wait();
            return Ok(if stopped {
                Once::Stopped
            } else {
                Once::TimedOut
            });
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
    services[0].log = fpm_log;
    services[1].log = caddy_log;
    let mut ready = Some(ready);
    // Tugas terjadwal yang sedang berjalan. Seperti cron: dijalankan tiap pergantian menit.
    // `schedule:run` Laravel tidak menunggu yang sebelumnya selesai; wp-cron menunggu.
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
            let minute = minute_now();
            if minute != last_minute && (task.overlap || runs.is_empty()) {
                last_minute = minute;
                match spawn(&task.name, shell_cmd(plan, &task.run), task.log) {
                    Ok(child) => runs.push(child),
                    Err(e) => say!("jamu: {} gagal dijalankan: {e}", task.name),
                }
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perintah_sekali_jalan_selesai_gagal_kehabisan_waktu_atau_dihentikan() {
        let out = std::env::temp_dir().join(format!("jamu-once-{}.log", std::process::id()));
        let run = |script: &str, budget: u64, stop: bool| {
            run_once(
                os::shell(script),
                &out,
                Duration::from_millis(budget),
                &AtomicBool::new(stop),
            )
            .unwrap()
        };
        assert!(matches!(run("echo halo", 5000, false), Once::Done));
        assert_eq!(fs::read_to_string(&out).unwrap().trim(), "halo");
        // Keluaran galat masuk ke berkas yang sama.
        assert!(
            matches!(run("echo aduh 1>&2 && exit 3", 5000, false), Once::Failed(s) if s.code() == Some(3))
        );
        assert_eq!(fs::read_to_string(&out).unwrap().trim(), "aduh");
        // `ping` ada di Linux dan Windows; dua-duanya menunggu sekitar 20 detik di sini.
        let lama = if cfg!(windows) {
            "ping -n 20 127.0.0.1"
        } else {
            "sleep 20"
        };
        let start = Instant::now();
        assert!(matches!(run(lama, 300, false), Once::TimedOut));
        assert!(matches!(run(lama, 60_000, true), Once::Stopped));
        assert!(start.elapsed() < Duration::from_secs(5));
        let _ = fs::remove_file(&out);
    }

    #[test]
    fn kabar_php_fpm_ditulis_ulang() {
        let timeout = r#"[14-Nov-2026 15:44:33] WARNING: [pool www] child 812, script '/app/impor.php' (request: "POST /impor.php?x=1") execution timed out (302.117 sec), terminating"#;
        assert_eq!(
            fpm_log("php-fpm", timeout).as_deref(),
            Some(
                "jamu: permintaan POST /impor.php?x=1 dihentikan: melewati batas waktu (timeout di jamu.toml)"
            )
        );
        let busy = "[14-Nov-2026 15:52:59] WARNING: [pool www] server reached max_children setting (8), consider raising it";
        assert!(fpm_log("php-fpm", busy).unwrap().contains("sedang sibuk"));
        let killed = "[14-Nov-2026 15:52:59] WARNING: [pool www] child 9 exited on signal 9 (SIGKILL) after 4.2 seconds from start";
        let text = fpm_log("php-fpm", killed).unwrap();
        assert!(text.starts_with("[php-fpm] [14-Nov-2026") && text.contains("memori container"));
        for routine in [
            "[14-Nov-2026 16:01:03] NOTICE: [pool www] child 2668 exited with code 0 after 0.634504 seconds from start",
            "[14-Nov-2026 16:01:03] NOTICE: [pool www] child 2672 started",
        ] {
            assert_eq!(fpm_log("php-fpm", routine), None);
        }
        // Pekerja yang keluar dengan galat tetap diberitakan.
        let crash = "[14-Nov-2026 16:01:03] WARNING: [pool www] child 7 exited with code 255 after 1.2 seconds from start";
        assert!(fpm_log("php-fpm", crash).is_some());
        let other = "[14-Nov-2026 15:44:33] NOTICE: ready to handle connections";
        assert_eq!(
            fpm_log("php-fpm", other),
            Some(format!("[php-fpm] {other}"))
        );
    }

    #[test]
    fn log_akses_diringkas_dan_baris_lain_dibiarkan() {
        let access = r#"{"level":"info","ts":1.5,"logger":"http.log.access.log0","msg":"handled request","request":{"remote_ip":"10.0.0.2","client_ip":"203.0.113.9","method":"GET","host":"situs.test","uri":"/wp-login.php?x=1","headers":{}},"duration":0.0123,"size":512,"status":200}"#;
        assert_eq!(
            caddy_log("caddy", access).as_deref(),
            Some("[akses] 203.0.113.9 GET /wp-login.php?x=1 200 12 ms")
        );
        let stderr = r#"{"level":"warn","logger":"http.handlers.reverse_proxy.fastcgi","msg":"stderr","body":"PHP Warning: x\nbaris dua"}"#;
        assert_eq!(
            caddy_log("caddy", stderr).as_deref(),
            Some("[php-cgi] PHP Warning: x\n[php-cgi] baris dua")
        );
        assert_eq!(
            caddy_log("caddy", "bukan json").as_deref(),
            Some("[caddy] bukan json")
        );
    }

    #[test]
    fn kabar_sertifikat_diringkas() {
        let log = |line: &str| caddy_log("caddy", line);
        // Dua baris ini disalin dari Caddy 2.11 yang tidak bisa menghubungi CA.
        let gagal = r#"{"level":"error","ts":1794659699.1332648,"logger":"tls.obtain","msg":"could not get certificate from issuer","identifier":"uji.invalid","issuer":"127.0.0.1:9-dir","error":"registering account [] with server: provisioning client: performing request: Get \"https://127.0.0.1:9/dir\": dial tcp 127.0.0.1:9: connect: connection refused"}"#;
        let text = log(gagal).unwrap();
        assert!(text.starts_with("jamu: sertifikat untuk uji.invalid belum terbit: registering"));
        let ulang = r#"{"level":"error","ts":1794659699.1333046,"logger":"tls.obtain","msg":"will retry","error":"[uji.invalid] Obtain: x","attempt":1,"retrying_in":60,"elapsed":0.506289221,"max_duration":2592000}"#;
        assert_eq!(
            log(ulang).as_deref(),
            Some("jamu: sertifikat dicoba lagi dalam 1 menit; situs tetap dilayani")
        );
        let lama = r#"{"level":"error","logger":"tls.renew","msg":"will retry","error":"x","attempt":9,"retrying_in":21600}"#;
        assert!(log(lama).unwrap().contains("dalam 6 jam"));

        let terbit = r#"{"level":"info","ts":1.0,"logger":"tls.obtain","msg":"certificate obtained successfully","identifier":"situs.com","issuer":"acme-v02.api.letsencrypt.org-directory"}"#;
        assert_eq!(
            log(terbit).as_deref(),
            Some("jamu: sertifikat untuk situs.com terbit")
        );
        let panjang = r#"{"level":"info","ts":1.0,"logger":"tls.renew","msg":"certificate renewed successfully","identifier":"situs.com","issuer":"x"}"#;
        assert_eq!(
            log(panjang).as_deref(),
            Some("jamu: sertifikat untuk situs.com diperpanjang")
        );

        let dns = r#"{"level":"error","ts":1.0,"logger":"tls.issuance.acme.acme_client","msg":"challenge failed","identifier":"situs.com","challenge_type":"http-01","problem":{"type":"urn:ietf:params:acme:error:dns","title":"","detail":"DNS problem: NXDOMAIN looking up A for situs.com","instance":"","subproblems":[]}}"#;
        assert_eq!(
            log(dns).as_deref(),
            Some(
                "jamu: pemeriksaan domain situs.com gagal: DNS domain itu belum benar (DNS problem: NXDOMAIN looking up A for situs.com)"
            )
        );
        let caa = r#"{"level":"error","logger":"tls.issuance.acme.acme_client","msg":"challenge failed","identifier":"situs.com","challenge_type":"http-01","problem":{"type":"urn:ietf:params:acme:error:caa","detail":"CAA record for situs.com prevents issuance"}}"#;
        assert!(
            log(caa)
                .unwrap()
                .contains("rekaman CAA domain itu melarang CA ini")
        );

        // Langkah kecil dan peringatan bawaan dibuang; galat lain tetap lewat apa adanya.
        let langkah = r#"{"level":"info","logger":"tls.obtain","msg":"acquiring lock","identifier":"situs.com"}"#;
        assert_eq!(log(langkah), None);
        let admin = r#"{"level":"warn","ts":1.0,"logger":"admin","msg":"admin endpoint disabled"}"#;
        assert_eq!(log(admin), None);
        let lain = r#"{"level":"error","logger":"tls","msg":"job failed","error":"x"}"#;
        assert_eq!(log(lain), Some(format!("[caddy] {lain}")));
        // "will retry" milik bagian lain Caddy bukan urusan sertifikat.
        let bukan =
            r#"{"level":"error","logger":"http.handlers.x","msg":"will retry","retrying_in":5}"#;
        assert_eq!(log(bukan), Some(format!("[caddy] {bukan}")));
    }
}
