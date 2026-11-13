//! Spike S1. Kode sekali buang: satu file, tanpa abstraksi, Linux saja.
//!
//! `jamu serve [DIR]` mengenali project PHP, menulis konfigurasi Caddy dan
//! php-fpm ke folder state di luar project, lalu menyalakan dan mengawasi
//! keduanya sampai dihentikan.

use std::io::{BufRead, BufReader, Read};
use std::net::{TcpListener, TcpStream};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use std::{env, fs, thread};

/// Menulis ke stderr tanpa panik bila stderr sudah ditutup (misalnya keluaran disalurkan ke `head`).
macro_rules! say {
    ($($arg:tt)*) => {{
        use std::io::Write;
        let _ = writeln!(std::io::stderr(), $($arg)*);
    }};
}

// ---------------------------------------------------------------- deteksi

#[derive(Debug, PartialEq, Clone, Copy)]
enum Kind {
    Laravel,
    WordPress,
    Php,
}

#[derive(Debug)]
struct Project {
    dir: PathBuf,
    root: PathBuf,
    kind: Kind,
    /// Batasan versi PHP dari composer.json, hanya untuk ditampilkan di S1.
    php_wanted: Option<String>,
}

fn detect(dir: &Path) -> Project {
    let has_index = |d: &Path| d.join("index.php").is_file() || d.join("index.html").is_file();
    let is_wp = |d: &Path| d.join("wp-load.php").is_file() || d.join("wp-config.php").is_file();

    // WordPress di folder itu sendiri menang atas subfolder bernama `public` dan sejenisnya.
    let root = if is_wp(dir) {
        dir.to_path_buf()
    } else {
        ["public", "web", "public_html", "htdocs"]
            .iter()
            .map(|s| dir.join(s))
            .find(|d| has_index(d))
            .unwrap_or_else(|| dir.to_path_buf())
    };
    let kind = if dir.join("artisan").is_file() {
        Kind::Laravel
    } else if is_wp(&root) {
        Kind::WordPress
    } else {
        Kind::Php
    };
    let php_wanted = fs::read_to_string(dir.join("composer.json"))
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v["require"]["php"].as_str().map(str::to_owned));

    Project { dir: dir.to_path_buf(), root, kind, php_wanted }
}

// ---------------------------------------------------------------- argumen

struct Opts {
    dir: PathBuf,
    port: Option<u16>,
    domain: Option<String>,
    email: Option<String>,
    acme_ca: Option<String>,
    acme_ca_root: Option<String>,
}

const USAGE: &str = "\
Pemakaian:
  jamu serve [DIR] [--port N]
  jamu serve [DIR] --domain NAMA [--email ALAMAT] [--acme-ca URL] [--acme-ca-root FILE]
  jamu detect [DIR]

Tanpa --domain: HTTP di 127.0.0.1, port 8080 atau port bebas berikutnya.
Dengan --domain: port 80 dan 443, sertifikat diterbitkan dan diperbarui otomatis.";

fn parse_opts(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts { dir: PathBuf::from("."), port: None, domain: None, email: None, acme_ca: None, acme_ca_root: None };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = |name: &str| it.next().cloned().ok_or(format!("{name} butuh nilai"));
        match a.as_str() {
            "--port" => o.port = Some(val("--port")?.parse().map_err(|_| "--port harus angka 1-65535")?),
            "--domain" => o.domain = Some(val("--domain")?),
            "--email" => o.email = Some(val("--email")?),
            "--acme-ca" => o.acme_ca = Some(val("--acme-ca")?),
            "--acme-ca-root" => o.acme_ca_root = Some(val("--acme-ca-root")?),
            s if s.starts_with("--") => return Err(format!("opsi tidak dikenal: {s}")),
            s => o.dir = PathBuf::from(s),
        }
    }
    o.dir = fs::canonicalize(&o.dir).map_err(|e| format!("folder {} tidak bisa dibuka: {e}", o.dir.display()))?;
    if !o.dir.is_dir() {
        return Err(format!("{} bukan folder", o.dir.display()));
    }
    Ok(o)
}

// ---------------------------------------------------------------- lingkungan

fn find_bin(env_name: &str, names: &[&str]) -> Result<PathBuf, String> {
    if let Some(p) = env::var_os(env_name).filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(p));
    }
    let path = env::var("PATH").unwrap_or_default();
    // php-fpm dari paket distro ada di /usr/sbin, yang sering tidak ada di PATH pengguna biasa.
    let dirs = path.split(':').chain(["/usr/sbin", "/usr/local/sbin", "/opt/jamuphp/bin"]);
    for dir in dirs {
        for name in names {
            let p = Path::new(dir).join(name);
            if p.is_file() {
                return Ok(p);
            }
        }
    }
    Err(format!("{} tidak ditemukan. Pasang, atau tunjuk lewat variabel {env_name}.", names[0]))
}

fn fnv1a(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c9dc5u32, |h, b| (h ^ *b as u32).wrapping_mul(0x01000193))
}

/// Folder state berada di luar project, supaya tidak ikut tersaji dan tidak mengotori repo.
fn state_dir(project: &Path) -> PathBuf {
    let base = env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
        .unwrap_or_else(|| PathBuf::from("/tmp"));
    let name: String = project
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let hash = fnv1a(project.as_os_str().as_encoded_bytes());
    base.join("jamuphp").join(format!("{name}-{hash:08x}"))
}

fn port_free(port: u16) -> bool {
    TcpListener::bind(("0.0.0.0", port)).is_ok()
}

/// Nama program yang mendengarkan di sebuah port, dicari lewat /proc. Ini usaha terbaik:
/// proses milik pengguna lain tidak terlihat tanpa hak root.
fn port_holder(port: u16) -> String {
    let listening_inode = |table: String| {
        table.lines().skip(1).find_map(|line| {
            // Kolom: sl, alamat lokal, alamat lawan, status (0A = LISTEN), ..., inode di indeks 9.
            let f: Vec<&str> = line.split_whitespace().collect();
            let local_port = u16::from_str_radix(f.get(1)?.rsplit(':').next()?, 16).ok()?;
            (local_port == port && f.get(3) == Some(&"0A")).then(|| f.get(9).map(|s| s.to_string()))?
        })
    };
    let inode = ["/proc/net/tcp", "/proc/net/tcp6"].iter().filter_map(|f| fs::read_to_string(f).ok()).find_map(listening_inode);
    let Some(inode) = inode else { return String::new() };
    let target = format!("socket:[{inode}]");
    for proc in fs::read_dir("/proc").into_iter().flatten().flatten() {
        let pid = proc.file_name().to_string_lossy().into_owned();
        if !pid.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        for fd in fs::read_dir(proc.path().join("fd")).into_iter().flatten().flatten() {
            if fs::read_link(fd.path()).is_ok_and(|l| l.to_string_lossy() == target) {
                let name = fs::read_to_string(proc.path().join("comm")).unwrap_or_default();
                return format!(" oleh {} (pid {pid})", name.trim());
            }
        }
    }
    String::new()
}

// ---------------------------------------------------------------- konfigurasi

struct Paths {
    caddy: PathBuf,
    fpm: PathBuf,
    caddyfile: PathBuf,
    fpm_conf: PathBuf,
    sock: PathBuf,
}

fn workers() -> usize {
    // Dua kali jumlah core, minimal 4 supaya permintaan WordPress ke dirinya sendiri tidak macet.
    (thread::available_parallelism().map(|n| n.get()).unwrap_or(2) * 2).max(4)
}

fn fpm_conf(sock: &Path, dev: bool) -> String {
    let as_root = unsafe { libc::geteuid() } == 0;
    let mut s = String::new();
    s += "[global]\nerror_log = /proc/self/fd/2\ndaemonize = no\n\n[www]\n";
    if as_root {
        s += "user = root\ngroup = root\n";
    }
    s += &format!("listen = {}\nlisten.mode = 0600\n", sock.display());
    s += &format!("pm = ondemand\npm.max_children = {}\npm.process_idle_timeout = 10s\n", workers());
    s += "catch_workers_output = yes\ndecorate_workers_output = no\nclear_env = no\n";
    s += "php_admin_value[memory_limit] = 256M\n";
    s += "php_admin_value[upload_max_filesize] = 128M\n";
    s += "php_admin_value[post_max_size] = 128M\n";
    s += &format!("php_admin_flag[display_errors] = {}\n", if dev { "on" } else { "off" });
    s
}

fn q(p: &Path) -> String {
    format!("\"{}\"", p.display().to_string().replace('\\', "\\\\").replace('"', "\\\""))
}

fn caddyfile(p: &Project, o: &Opts, port: u16, state: &Path, sock: &Path) -> String {
    let mut s = String::from("{\n\tadmin off\n\tpersist_config off\n");
    s += &format!("\tstorage file_system {}\n", q(&state.join("caddy")));
    s += "\tlog {\n\t\tlevel WARN\n\t}\n";
    match &o.domain {
        None => s += "\tauto_https off\n",
        Some(_) => {
            if let Some(e) = &o.email {
                s += &format!("\temail {e}\n");
            }
            if let Some(ca) = &o.acme_ca {
                s += &format!("\tacme_ca {ca}\n");
            }
            if let Some(root) = &o.acme_ca_root {
                s += &format!("\tacme_ca_root {}\n", q(Path::new(root)));
            }
        }
    }
    s += "}\n\n";

    match &o.domain {
        // Dev hanya mendengarkan di loopback: situs lokal tidak boleh terbuka ke jaringan.
        None => s += &format!("http://:{port} {{\n\tbind 127.0.0.1\n"),
        Some(d) => s += &format!("{d} {{\n"),
    }
    s += &format!("\troot * {}\n", q(&p.root));
    s += "\trequest_body {\n\t\tmax_size 128MB\n\t}\n";
    // Dotfile (.env, .git, .htaccess) tidak pernah disajikan, kecuali .well-known.
    s += "\t@hidden {\n\t\tpath_regexp (^|/)\\.\n\t\tnot path /.well-known/*\n\t}\n\trespond @hidden 404\n";
    if p.kind == Kind::WordPress {
        s += "\t@uploads_php path_regexp (?i)^/wp-content/uploads/.*\\.php(/|$)\n\trespond @uploads_php 403\n";
    }
    s += "\tencode zstd gzip\n";
    // flush_interval -1: keluaran PHP diteruskan saat itu juga, tidak ditampung dulu.
    // Tanpa ini respons yang dialirkan (SSE, unduhan yang dibangkitkan) baru tiba setelah selesai.
    s += &format!("\tphp_fastcgi {} {{\n\t\tflush_interval -1\n\t}}\n", q(Path::new(&format!("unix/{}", sock.display()))));
    s += "\tfile_server\n}\n";
    s
}

// ---------------------------------------------------------------- proses

fn spawn(name: &'static str, mut cmd: Command) -> std::io::Result<Child> {
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    // Grup proses sendiri: Ctrl-C di terminal hanya sampai ke supervisor, yang lalu
    // mematikan anak-anaknya secara berurutan.
    cmd.process_group(0);
    // Bila supervisor mati mendadak, kernel mengirim SIGTERM ke anak.
    unsafe {
        cmd.pre_exec(|| {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM);
            Ok(())
        });
    }
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

fn caddy_cmd(p: &Paths) -> Command {
    let mut c = Command::new(&p.caddy);
    c.args(["run", "--adapter", "caddyfile", "--config"]).arg(&p.caddyfile);
    c
}

fn fpm_cmd(p: &Paths) -> Command {
    let mut c = Command::new(&p.fpm);
    c.args(["--nodaemonize", "--fpm-config"]).arg(&p.fpm_conf);
    if unsafe { libc::geteuid() } == 0 {
        c.arg("--allow-to-run-as-root");
    }
    c
}

struct Service {
    name: &'static str,
    make: fn(&Paths) -> Command,
    /// Sinyal untuk berhenti dengan rapi.
    stop_signal: i32,
    child: Option<Child>,
    started: Instant,
    backoff: Duration,
    retry_at: Instant,
}

fn signal_group(child: &Child, sig: i32) {
    unsafe { libc::kill(-(child.id() as i32), sig) };
}

fn supervise(paths: &Paths, stop: &AtomicBool, ready: impl FnOnce()) -> Result<(), String> {
    let now = Instant::now();
    let new = |name, make, stop_signal| Service {
        name,
        make,
        stop_signal,
        child: None,
        started: now,
        backoff: Duration::from_secs(1),
        retry_at: now,
    };
    // php-fpm lebih dulu, supaya soketnya ada saat Caddy mulai menerima permintaan.
    let mut services =
        [new("php-fpm", fpm_cmd as fn(&Paths) -> Command, libc::SIGQUIT), new("caddy", caddy_cmd, libc::SIGTERM)];
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
                        say!("jamu: {} berhenti ({status}); dinyalakan ulang dalam {} detik", s.name, s.backoff.as_secs());
                        // Anak yang tertinggal, misalnya pekerja php-fpm yang induknya mati, masih
                        // memegang soket. Bersihkan seluruh grup proses sebelum menyalakan ulang.
                        signal_group(child, libc::SIGKILL);
                        if s.name == "php-fpm" {
                            let _ = fs::remove_file(&paths.sock);
                        }
                        s.child = None;
                        s.retry_at = Instant::now() + s.backoff;
                        s.backoff = (s.backoff * 2).min(Duration::from_secs(30));
                    }
                }
                None if Instant::now() >= s.retry_at => {
                    let child = spawn(s.name, (s.make)(paths)).map_err(|e| format!("gagal menyalakan {}: {e}", s.name))?;
                    s.child = Some(child);
                    s.started = Instant::now();
                }
                None => {}
            }
        }
        if services.iter().all(|s| s.child.is_some()) {
            if let Some(f) = ready.take() {
                f();
            }
        }
        thread::sleep(Duration::from_millis(200));
    }

    say!("jamu: berhenti...");
    for s in services.iter().rev() {
        if let Some(child) = &s.child {
            unsafe { libc::kill(child.id() as i32, s.stop_signal) };
        }
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    for s in services.iter_mut() {
        let Some(child) = &mut s.child else { continue };
        while child.try_wait().ok().flatten().is_none() {
            if Instant::now() > deadline {
                say!("jamu: {} tidak mau berhenti; dimatikan paksa", s.name);
                signal_group(child, libc::SIGKILL);
                let _ = child.wait();
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
    }
    let _ = fs::remove_file(&paths.sock);
    Ok(())
}

// ---------------------------------------------------------------- perintah

fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::Laravel => "Laravel",
        Kind::WordPress => "WordPress",
        Kind::Php => "PHP",
    }
}

fn serve(o: Opts) -> Result<(), String> {
    let project = detect(&o.dir);
    let caddy = find_bin("JAMU_CADDY", &["caddy"])?;
    let fpm = find_bin("JAMU_PHP_FPM", &["php-fpm", "php-fpm8.5", "php-fpm8.4", "php-fpm8.3", "php-fpm8.2"])?;

    let port = match (&o.domain, o.port) {
        (Some(_), _) => {
            for p in [80, 443] {
                if !port_free(p) {
                    return Err(format!("Port {p} sedang dipakai{}. Mode domain butuh port 80 dan 443.", port_holder(p)));
                }
            }
            443
        }
        (None, Some(p)) if port_free(p) => p,
        (None, Some(p)) => return Err(format!("Port {p} sedang dipakai{}.", port_holder(p))),
        (None, None) => {
            let p = (8080..8180).find(|p| port_free(*p)).ok_or("Tidak ada port bebas di 8080-8179.")?;
            if p != 8080 {
                say!("jamu: port 8080 sedang dipakai{}; memakai {p}", port_holder(8080));
            }
            p
        }
    };

    let state = state_dir(&project.dir);
    fs::create_dir_all(state.join("caddy")).map_err(|e| format!("tidak bisa membuat {}: {e}", state.display()))?;
    // Panjang path soket Unix dibatasi kernel (108 byte).
    let mut sock = state.join("php-fpm.sock");
    if sock.as_os_str().len() > 100 {
        sock = PathBuf::from(format!("/tmp/jamu-{:08x}.sock", fnv1a(state.as_os_str().as_encoded_bytes())));
    }
    let _ = fs::remove_file(&sock);
    let paths = Paths { caddy, fpm, caddyfile: state.join("Caddyfile"), fpm_conf: state.join("php-fpm.conf"), sock };
    let write = |p: &Path, text: String| fs::write(p, text).map_err(|e| format!("tidak bisa menulis {}: {e}", p.display()));
    write(&paths.fpm_conf, fpm_conf(&paths.sock, o.domain.is_none()))?;
    write(&paths.caddyfile, caddyfile(&project, &o, port, &state, &paths.sock))?;

    let php_version = Command::new(&paths.fpm)
        .arg("-v")
        .output()
        .ok()
        .and_then(|out| String::from_utf8_lossy(&out.stdout).split_whitespace().nth(1).map(str::to_owned))
        .unwrap_or_else(|| "?".into());
    let rel_root = project.root.strip_prefix(&project.dir).ok().filter(|r| !r.as_os_str().is_empty());
    say!(
        "jamu: {} di {} (document root: {})",
        kind_name(project.kind),
        project.dir.display(),
        rel_root.map(|r| r.display().to_string()).unwrap_or_else(|| ".".into())
    );
    say!(
        "jamu: PHP {php_version}{}, php-fpm ondemand, maksimal {} pekerja",
        project.php_wanted.as_ref().map(|w| format!(" (composer.json meminta {w})")).unwrap_or_default(),
        workers()
    );
    say!("jamu: konfigurasi di {}", state.display());

    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    ctrlc::set_handler(move || flag.store(true, Ordering::SeqCst)).map_err(|e| e.to_string())?;

    let url = match &o.domain {
        Some(d) => format!("https://{d}"),
        None => format!("http://localhost:{port}"),
    };
    let domain_mode = o.domain.is_some();
    supervise(&paths, &stop, move || {
        // Tunggu sampai port benar-benar menerima koneksi sebelum mengumumkan siap.
        thread::spawn(move || {
            let until = Instant::now() + Duration::from_secs(15);
            while Instant::now() < until {
                if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                    let note = if domain_mode { " (sertifikat diterbitkan dan diperbarui otomatis)" } else { "" };
                    say!("jamu: siap di {url}{note}");
                    return;
                }
                thread::sleep(Duration::from_millis(100));
            }
            say!("jamu: port {port} belum menerima koneksi setelah 15 detik; periksa log di atas");
        });
    })
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("serve") => parse_opts(&args[1..]).and_then(serve),
        Some("detect") => parse_opts(&args[1..]).map(|o| {
            let p = detect(&o.dir);
            println!("jenis: {}", kind_name(p.kind));
            println!("document root: {}", p.root.display());
            println!("php diminta: {}", p.php_wanted.as_deref().unwrap_or("-"));
        }),
        _ => Err(USAGE.to_string()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            say!("{e}");
            ExitCode::FAILURE
        }
    }
}

// ---------------------------------------------------------------- tes

#[cfg(test)]
mod tests {
    use super::*;

    fn project(files: &[&str]) -> PathBuf {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = env::temp_dir().join(format!("jamu-test-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
        for f in files {
            let p = dir.join(f);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(p, if f.ends_with("composer.json") { r#"{"require":{"php":"^8.3"}}"# } else { "" }).unwrap();
        }
        dir
    }

    #[test]
    fn laravel_memakai_public_dan_membaca_versi_php() {
        let dir = project(&["artisan", "public/index.php", "composer.json"]);
        let p = detect(&dir);
        assert_eq!(p.kind, Kind::Laravel);
        assert_eq!(p.root, dir.join("public"));
        assert_eq!(p.php_wanted.as_deref(), Some("^8.3"));
    }

    #[test]
    fn wordpress_di_folder_itu_sendiri_walau_ada_subfolder_public() {
        let dir = project(&["wp-load.php", "index.php", "public/index.html"]);
        let p = detect(&dir);
        assert_eq!(p.kind, Kind::WordPress);
        assert_eq!(p.root, dir);
    }

    #[test]
    fn wordpress_di_subfolder_web() {
        let dir = project(&["web/index.php", "web/wp-config.php"]);
        let p = detect(&dir);
        assert_eq!(p.kind, Kind::WordPress);
        assert_eq!(p.root, dir.join("web"));
    }

    #[test]
    fn php_umum_memakai_folder_itu_sendiri() {
        let dir = project(&["index.php"]);
        let p = detect(&dir);
        assert_eq!(p.kind, Kind::Php);
        assert_eq!(p.root, dir);
        assert_eq!(p.php_wanted, None);
    }

    #[test]
    fn subfolder_tanpa_index_diabaikan() {
        let dir = project(&["public/readme.txt", "index.php"]);
        assert_eq!(detect(&dir).root, dir);
    }

    #[test]
    fn caddyfile_dev_hanya_di_loopback_dan_menolak_dotfile() {
        let dir = project(&["wp-load.php", "index.php"]);
        let o = Opts { dir: dir.clone(), port: None, domain: None, email: None, acme_ca: None, acme_ca_root: None };
        let text = caddyfile(&detect(&dir), &o, 8080, Path::new("/s"), Path::new("/s/php-fpm.sock"));
        assert!(text.contains("bind 127.0.0.1"));
        assert!(text.contains("auto_https off"));
        assert!(text.contains("respond @hidden 404"));
        assert!(text.contains("respond @uploads_php 403"));
    }

    #[test]
    fn caddyfile_domain_tidak_mematikan_https() {
        let dir = project(&["index.php"]);
        let o = Opts { dir: dir.clone(), port: None, domain: Some("situs.test".into()), email: None, acme_ca: None, acme_ca_root: None };
        let text = caddyfile(&detect(&dir), &o, 443, Path::new("/s"), Path::new("/s/php-fpm.sock"));
        assert!(text.contains("situs.test {"));
        assert!(!text.contains("auto_https off"));
        assert!(!text.contains("@uploads_php"));
    }
}
