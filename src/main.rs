//! Spike S1-S3. Kode sekali buang: satu file, tanpa abstraksi.
//!
//! `jamu serve [DIR]` mengenali project PHP, menulis konfigurasi Caddy dan PHP ke folder
//! state di luar project, lalu menyalakan dan mengawasi keduanya sampai dihentikan.
//!
//! Di Linux PHP dijalankan php-fpm lewat soket Unix. Di Windows tidak ada php-fpm, jadi
//! yang dijalankan adalah sekumpulan php-cgi, masing-masing di port loopback sendiri.
//! Semua yang berbeda antar-OS ada di modul `os`.

use std::io::{BufRead, BufReader, Read};
use std::net::{TcpListener, TcpStream};
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
    behind_proxy: bool,
}

const USAGE: &str = "\
Pemakaian:
  jamu serve [DIR] [--port N]
  jamu serve [DIR] --domain NAMA [--email ALAMAT] [--acme-ca URL] [--acme-ca-root FILE]
  jamu serve [DIR] --behind-proxy [--port N]
  jamu health [--port N]
  jamu detect [DIR]

Tanpa opsi: HTTP di 127.0.0.1, port 8080 atau port bebas berikutnya.
Dengan --domain: port 80 dan 443, sertifikat diterbitkan dan diperbarui otomatis.
Dengan --behind-proxy: HTTP di semua antarmuka, port 8080, untuk container di belakang
Traefik atau proxy lain yang mengurus HTTPS.";

fn parse_opts(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts { dir: PathBuf::from("."), port: None, domain: None, email: None, acme_ca: None, acme_ca_root: None, behind_proxy: false };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = |name: &str| it.next().cloned().ok_or(format!("{name} butuh nilai"));
        match a.as_str() {
            "--port" => o.port = Some(val("--port")?.parse().map_err(|_| "--port harus angka 1-65535")?),
            "--domain" => o.domain = Some(val("--domain")?),
            "--behind-proxy" => o.behind_proxy = true,
            "--email" => o.email = Some(val("--email")?),
            "--acme-ca" => o.acme_ca = Some(val("--acme-ca")?),
            "--acme-ca-root" => o.acme_ca_root = Some(val("--acme-ca-root")?),
            s if s.starts_with("--") => return Err(format!("opsi tidak dikenal: {s}")),
            s => o.dir = PathBuf::from(s),
        }
    }
    if o.behind_proxy && o.domain.is_some() {
        return Err("--behind-proxy dan --domain tidak bisa dipakai bersamaan: pilih siapa yang mengurus HTTPS".into());
    }
    o.dir = fs::canonicalize(&o.dir).map(os::tidy).map_err(|e| format!("folder {} tidak bisa dibuka: {e}", o.dir.display()))?;
    if !o.dir.is_dir() {
        return Err(format!("{} bukan folder", o.dir.display()));
    }
    Ok(o)
}

// ---------------------------------------------------------------- lingkungan

/// Cara meminta sebuah proses berhenti dengan rapi.
#[derive(Clone, Copy)]
enum Stop {
    /// Caddy: selesaikan permintaan yang sedang berjalan, lalu keluar.
    Term,
    /// PHP: SIGQUIT di Linux, dimatikan di Windows.
    Quit,
}

#[cfg(unix)]
mod os {
    use super::Stop;
    use std::os::unix::process::CommandExt;
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command};
    use std::{env, fs};

    /// Folder tambahan untuk mencari binary: php-fpm dari paket distro ada di /usr/sbin,
    /// yang sering tidak ada di PATH pengguna biasa.
    pub const BIN_DIRS: &[&str] = &["/usr/sbin", "/usr/local/sbin", "/opt/jamuphp/bin"];

    pub fn is_root() -> bool {
        unsafe { libc::geteuid() == 0 }
    }

    pub fn tidy(p: PathBuf) -> PathBuf {
        p
    }

    /// Folder state berada di luar project, supaya tidak ikut tersaji dan tidak mengotori repo.
    pub fn state_base() -> PathBuf {
        env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("jamuphp")
    }

    /// Container dengan filesystem read-only, atau UID yang tidak punya folder sendiri:
    /// pakai /tmp, dalam folder yang hanya bisa dibuka pengguna ini.
    pub fn fallback_state_base() -> Option<PathBuf> {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt};
        let uid = unsafe { libc::geteuid() };
        let tmp = PathBuf::from(format!("/tmp/jamuphp-{uid}"));
        fs::DirBuilder::new().recursive(true).mode(0o700).create(&tmp).ok()?;
        (fs::metadata(&tmp).ok()?.uid() == uid).then_some(tmp)
    }

    pub fn prepare(cmd: &mut Command) {
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
    }

    pub fn init() {}

    pub fn ask_to_stop(child: &mut Child, how: Stop) {
        let sig = match how {
            Stop::Term => libc::SIGTERM,
            Stop::Quit => libc::SIGQUIT,
        };
        unsafe { libc::kill(child.id() as i32, sig) };
    }

    /// Mematikan seluruh grup proses, termasuk anak yang tertinggal.
    pub fn kill_tree(child: &mut Child) {
        unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
    }

    /// Sebagai PID 1 di container, proses yatim (misalnya pekerja php-fpm yang induknya mati,
    /// atau proses latar yang dijalankan kode situs) menjadi anak kita. Tanpa dipungut, mereka
    /// menumpuk sebagai zombie. Anak milik kita sendiri dilewati: itu urusan `try_wait`.
    pub fn reap_orphans(ours: &[u32]) {
        if unsafe { libc::getpid() } != 1 {
            return;
        }
        loop {
            let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
            // WNOWAIT: intip dulu siapa yang selesai, tanpa memungutnya.
            let r = unsafe { libc::waitid(libc::P_ALL, 0, &mut info, libc::WEXITED | libc::WNOHANG | libc::WNOWAIT) };
            let pid = unsafe { info.si_pid() };
            if r != 0 || pid == 0 || ours.contains(&(pid as u32)) {
                return;
            }
            unsafe { libc::waitpid(pid, std::ptr::null_mut(), libc::WNOHANG) };
        }
    }

    /// Alasan sebuah folder tidak bisa ditulis PHP, bila ada.
    pub fn unwritable(dir: &Path) -> Option<String> {
        use std::os::unix::ffi::OsStrExt;
        let c = std::ffi::CString::new(dir.as_os_str().as_bytes()).ok()?;
        (unsafe { libc::access(c.as_ptr(), libc::W_OK) } != 0).then(|| format!("uid {}", unsafe { libc::geteuid() }))
    }

    /// Nama program yang mendengarkan di sebuah port, dicari lewat /proc. Ini usaha terbaik:
    /// proses milik pengguna lain tidak terlihat tanpa hak root.
    pub fn port_holder(port: u16) -> String {
        let listening_inode = |table: String| {
            table.lines().skip(1).find_map(|line| {
                // Kolom: sl, alamat lokal, alamat lawan, status (0A = LISTEN), ..., inode di indeks 9.
                let f: Vec<&str> = line.split_whitespace().collect();
                let local_port = u16::from_str_radix(f.get(1)?.rsplit(':').next()?, 16).ok()?;
                (local_port == port && f.get(3) == Some(&"0A")).then(|| f.get(9).map(|s| s.to_string()))?
            })
        };
        let inode =
            ["/proc/net/tcp", "/proc/net/tcp6"].iter().filter_map(|f| fs::read_to_string(f).ok()).find_map(listening_inode);
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
}

#[cfg(windows)]
mod os {
    use super::Stop;
    use std::os::windows::process::CommandExt;
    use std::path::{Path, PathBuf};
    use std::process::{Child, Command};
    use std::{env, ffi::c_void};
    use windows_sys::Win32::System::Console::{CTRL_BREAK_EVENT, GenerateConsoleCtrlEvent};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JobObjectExtendedLimitInformation, SetInformationJobObject,
    };

    pub const BIN_DIRS: &[&str] = &[];

    pub fn is_root() -> bool {
        false
    }

    /// `canonicalize` di Windows menghasilkan `\\?\C:\...`. Caddy dan PHP tidak mengerti
    /// awalan itu, jadi dibuang untuk path drive biasa.
    pub fn tidy(p: PathBuf) -> PathBuf {
        match p.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
            Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
            _ => p,
        }
    }

    pub fn state_base() -> PathBuf {
        env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(env::temp_dir).join("jamuphp")
    }

    pub fn fallback_state_base() -> Option<PathBuf> {
        None
    }

    /// Supervisor masuk ke Job Object dengan KILL_ON_JOB_CLOSE, dan setiap proses yang
    /// dinyalakannya ikut di dalamnya. Saat supervisor mati, dengan cara apa pun, Windows
    /// menutup handle job dan mematikan semua yang tersisa. Ini padanan PR_SET_PDEATHSIG.
    pub fn init() {
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let size = std::mem::size_of_val(&info) as u32;
            SetInformationJobObject(job, JobObjectExtendedLimitInformation, &info as *const _ as *const c_void, size);
            // Handle sengaja tidak ditutup: ia harus hidup selama supervisor hidup.
            AssignProcessToJobObject(job, GetCurrentProcess());
        }
    }

    pub fn prepare(cmd: &mut Command) {
        // CREATE_NEW_PROCESS_GROUP: Ctrl-C di konsol hanya sampai ke supervisor, dan
        // Ctrl-Break bisa dikirim ke satu anak saja.
        cmd.creation_flags(0x0000_0200);
    }

    /// Windows tidak punya sinyal. Caddy mengartikan Ctrl-Break sebagai permintaan berhenti
    /// dengan rapi; php-cgi langsung dimatikan, dan Job Object miliknya ikut mematikan pekerjanya.
    pub fn ask_to_stop(child: &mut Child, how: Stop) {
        let asked = matches!(how, Stop::Term) && unsafe { GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, child.id()) } != 0;
        if !asked {
            let _ = child.kill();
        }
    }

    pub fn kill_tree(child: &mut Child) {
        let _ = child.kill();
    }

    pub fn reap_orphans(_ours: &[u32]) {}

    pub fn unwritable(_dir: &Path) -> Option<String> {
        None
    }

    pub fn port_holder(_port: u16) -> String {
        String::new()
    }
}

fn find_bin(env_name: &str, names: &[&str]) -> Result<PathBuf, String> {
    if let Some(p) = env::var_os(env_name).filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(p));
    }
    // Yang dibawa bersama jamu (di folder yang sama, atau subfolder `php`) menang atas PATH.
    let beside = env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf));
    let bundled = beside.iter().flat_map(|d| [d.clone(), d.join("php")]);
    let path = env::var_os("PATH").unwrap_or_default();
    for dir in bundled.chain(env::split_paths(&path)).chain(os::BIN_DIRS.iter().map(PathBuf::from)) {
        for name in names {
            let p = dir.join(format!("{name}{}", env::consts::EXE_SUFFIX));
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
    TcpListener::bind((if loopback_only { "127.0.0.1" } else { "0.0.0.0" }, port)).is_ok()
}

/// Port loopback bebas untuk php-cgi.
fn free_port() -> Result<u16, String> {
    let l = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| e.to_string())?;
    l.local_addr().map(|a| a.port()).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------- konfigurasi

/// Cara PHP dijalankan.
enum Php {
    /// Linux: php-fpm lewat soket Unix. Pekerja dibuat saat ada permintaan.
    Fpm { bin: PathBuf, conf: PathBuf, sock: PathBuf },
    /// Windows, yang tidak punya php-fpm: php-cgi sebagai server FastCGI di port loopback.
    /// Dengan PHP_FCGI_CHILDREN ia sendiri yang membuat pekerja, membagi satu soket ke
    /// semuanya, dan mengganti pekerja yang keluar. Supervisor cukup mengawasi induknya.
    Cgi { bin: PathBuf, ini: PathBuf, port: u16 },
}

struct Plan {
    caddy: PathBuf,
    caddyfile: PathBuf,
    php: Php,
}

/// Dijawab php-fpm sendiri ("pong"), tanpa menjalankan kode situs. Caddy hanya meneruskannya
/// untuk permintaan dari loopback, jadi `jamu health` menguji Caddy dan php-fpm sekaligus.
const PING_PATH: &str = "/-/jamu/ping";

fn workers() -> usize {
    // Dua kali jumlah core, minimal 4 supaya permintaan WordPress ke dirinya sendiri tidak macet.
    (thread::available_parallelism().map(|n| n.get()).unwrap_or(2) * 2).max(4)
}

/// Pekerja php-cgi selalu hidup semua (tidak ada ondemand), jadi jumlahnya dibatasi.
fn cgi_workers() -> usize {
    workers().min(8)
}

fn fpm_conf(sock: &Path, dev: bool) -> String {
    let mut s = String::new();
    s += "[global]\nerror_log = /proc/self/fd/2\ndaemonize = no\n\n[www]\n";
    if os::is_root() {
        s += "user = root\ngroup = root\n";
    }
    s += &format!("listen = {}\nlisten.mode = 0600\n", sock.display());
    s += &format!("pm = ondemand\npm.max_children = {}\npm.process_idle_timeout = 10s\n", workers());
    s += "catch_workers_output = yes\ndecorate_workers_output = no\nclear_env = no\n";
    s += &format!("ping.path = {PING_PATH}\n");
    s += "php_admin_value[memory_limit] = 256M\n";
    s += "php_admin_value[upload_max_filesize] = 128M\n";
    s += "php_admin_value[post_max_size] = 128M\n";
    s += &format!("php_admin_flag[display_errors] = {}\n", if dev { "on" } else { "off" });
    s
}

/// Ekstensi dari zip resmi PHP untuk Windows yang dinyalakan bila filenya ada.
/// Urutan penting: exif butuh mbstring dimuat lebih dulu.
const WINDOWS_EXTENSIONS: &[&str] = &[
    "bz2", "curl", "fileinfo", "ftp", "gd", "gettext", "gmp", "intl", "mbstring", "exif", "mysqli", "openssl", "pdo_mysql",
    "pdo_pgsql", "pdo_sqlite", "pgsql", "soap", "sockets", "sodium", "sqlite3", "xsl", "zip",
];

/// Zip resmi PHP untuk Windows datang tanpa php.ini: tidak satu pun ekstensi aktif, dan
/// tidak ada daftar CA, sehingga HTTPS dari PHP gagal. Keduanya diisi dari sini.
fn windows_ini(php_dir: &Path) -> String {
    let fwd = |p: PathBuf| p.display().to_string().replace('\\', "/");
    let ext = php_dir.join("ext");
    let has = |name: &str| ext.join(format!("php_{name}.dll")).is_file();
    let mut s = format!("extension_dir = \"{}\"\n", fwd(ext.clone()));
    for name in WINDOWS_EXTENSIONS.iter().filter(|n| has(n)) {
        s += &format!("extension = {name}\n");
    }
    // Sejak PHP 8.5 opcache menyatu di dalam PHP dan tidak punya DLL.
    if has("opcache") {
        s += "zend_extension = opcache\n";
    }
    let ca = php_dir.join("extras/ssl/cacert.pem");
    if ca.is_file() {
        s += &format!("curl.cainfo = \"{0}\"\nopenssl.cafile = \"{0}\"\n", fwd(ca));
    }
    s
}

/// php-cgi tidak punya `php_admin_value` seperti php-fpm, jadi pengaturannya lewat php.ini
/// yang dibangkitkan dan ditunjuk dengan `-c`.
fn cgi_ini(bin: &Path, dev: bool) -> String {
    let mut s = String::from("memory_limit = 256M\nupload_max_filesize = 128M\npost_max_size = 128M\n");
    s += &format!("display_errors = {}\nlog_errors = On\n", if dev { "On" } else { "Off" });
    if cfg!(windows) {
        s += &windows_ini(bin.parent().unwrap_or(Path::new(".")));
    }
    s
}

/// Path dalam tanda kutip untuk Caddyfile. Di dalam kutip Caddy hanya mengenal `\"`;
/// garis miring terbalik lain dibiarkan apa adanya. Path Windows ditulis dengan garis miring
/// biasa, yang diterima Caddy maupun PHP.
fn q(p: &Path) -> String {
    let s = p.display().to_string();
    let s = if cfg!(windows) { s.replace('\\', "/") } else { s };
    format!("\"{}\"", s.replace('"', "\\\""))
}

fn caddyfile(p: &Project, o: &Opts, port: u16, state: &Path, php: &Php) -> String {
    let mut s = String::from("{\n\tadmin off\n\tpersist_config off\n");
    s += &format!("\tstorage file_system {}\n", q(&state.join("caddy")));
    s += "\tlog {\n\t\tlevel WARN\n\t}\n";
    if o.domain.is_none() {
        // Tanpa TLS hanya HTTP/1.1 yang berlaku; menyebutnya terang-terangan membungkam
        // peringatan Caddy soal HTTP/2 dan HTTP/3 di setiap start.
        s += "\tservers {\n\t\tprotocols h1\n";
        if o.behind_proxy {
            // Alamat klien diambil dari X-Forwarded-For hanya bila pengirimnya proxy di jaringan privat.
            s += "\t\ttrusted_proxies static private_ranges\n";
        }
        s += "\t}\n";
    }
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
        None if o.behind_proxy => s += &format!("http://:{port} {{\n"),
        None => s += &format!("http://:{port} {{\n\tbind 127.0.0.1\n"),
        Some(d) => s += &format!("{d} {{\n"),
    }
    s += &format!("\troot * {}\n", q(&p.root));
    s += "\trequest_body {\n\t\tmax_size 128MB\n\t}\n";
    // Dotfile (.env, .git, .htaccess) tidak pernah disajikan, kecuali .well-known.
    s += "\t@hidden {\n\t\tpath_regexp (^|/)\\.\n\t\tnot path /.well-known/*\n\t}\n\trespond @hidden 404\n";
    if p.kind == Kind::WordPress {
        // Windows mengabaikan titik dan spasi di ujung nama file, jadi `x.php.` adalah `x.php`.
        // Spasi ditulis \\x20 karena spasi biasa memotong token di Caddyfile.
        s += "\t@uploads_php path_regexp (?i)^/wp-content/uploads/.*\\.php[.\\x20]*(/|$)\n\trespond @uploads_php 403\n";
    }
    let upstream = match php {
        Php::Fpm { sock, .. } => q(Path::new(&format!("unix/{}", sock.display()))),
        Php::Cgi { port, .. } => format!("127.0.0.1:{port}"),
    };
    if let Php::Fpm { .. } = php {
        s += &format!("\t@ping {{\n\t\tpath {PING_PATH}\n\t\tremote_ip 127.0.0.1 ::1\n\t}}\n");
        // php-fpm mengenali ping dari SCRIPT_NAME; tanpa split_path Caddy mengirimnya kosong.
        s += &format!(
            "\treverse_proxy @ping {upstream} {{\n\t\ttransport fastcgi {{\n\t\t\tenv SCRIPT_NAME {PING_PATH}\n\t\t\tenv SCRIPT_FILENAME {PING_PATH}\n\t\t}}\n\t}}\n"
        );
    }
    s += "\tencode zstd gzip\n";
    // flush_interval -1: keluaran PHP diteruskan saat itu juga, tidak ditampung dulu.
    // Tanpa ini respons yang dialirkan (SSE, unduhan yang dibangkitkan) baru tiba setelah selesai.
    if o.behind_proxy {
        // HTTPS berakhir di proxy. Tanpa HTTPS=on, WordPress mengira situsnya HTTP lalu
        // berputar-putar dalam redirect. Hanya dipercaya bila pengirimnya di jaringan privat.
        // REMOTE_ADDR diisi alamat pengunjung, bukan alamat proxy.
        s += "\t@fwd_https {\n\t\theader X-Forwarded-Proto https\n\t\tremote_ip private_ranges\n\t}\n";
        s += &format!(
            "\tphp_fastcgi @fwd_https {upstream} {{\n\t\tflush_interval -1\n\t\tenv HTTPS on\n\t\tenv REMOTE_ADDR {{client_ip}}\n\t}}\n"
        );
        s += &format!("\tphp_fastcgi {upstream} {{\n\t\tflush_interval -1\n\t\tenv REMOTE_ADDR {{client_ip}}\n\t}}\n");
    } else {
        s += &format!("\tphp_fastcgi {upstream} {{\n\t\tflush_interval -1\n\t}}\n");
    }
    s += "\tfile_server\n}\n";
    s
}

// ---------------------------------------------------------------- proses

fn spawn(name: &'static str, mut cmd: Command) -> std::io::Result<Child> {
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
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
    c.args(["run", "--adapter", "caddyfile", "--config"]).arg(&plan.caddyfile);
    // Caddy menyimpan catatan kecil di folder data dan config pengguna. Arahkan ke folder
    // state supaya tidak menyentuh HOME, yang di container sering tidak bisa ditulis.
    if let Some(state) = plan.caddyfile.parent() {
        c.env("XDG_DATA_HOME", state.join("caddy-data")).env("XDG_CONFIG_HOME", state.join("caddy-config"));
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
            c.arg("-b").arg(format!("127.0.0.1:{port}")).arg("-c").arg(ini);
            // Pekerja diganti setelah 500 permintaan, supaya kebocoran memori tidak menumpuk.
            c.env("PHP_FCGI_CHILDREN", cgi_workers().to_string()).env("PHP_FCGI_MAX_REQUESTS", "500");
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

fn supervise(plan: &Plan, stop: &AtomicBool, ready: impl FnOnce()) -> Result<(), String> {
    let now = Instant::now();
    let new = |name, make, stop| Service { name, make, stop, child: None, started: now, backoff: Duration::from_secs(1), retry_at: now };
    let (php_name, sock) = match &plan.php {
        Php::Fpm { sock, .. } => ("php-fpm", Some(sock)),
        Php::Cgi { .. } => ("php-cgi", None),
    };
    // PHP lebih dulu, supaya sudah mendengarkan saat Caddy mulai menerima permintaan.
    let mut services = [new(php_name, php_cmd as fn(&Plan) -> Command, Stop::Quit), new("caddy", caddy_cmd, Stop::Term)];
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
                    let child = spawn(s.name, (s.make)(plan)).map_err(|e| format!("gagal menyalakan {}: {e}", s.name))?;
                    s.child = Some(child);
                    s.started = Instant::now();
                }
                None => {}
            }
        }
        os::reap_orphans(&services.iter().filter_map(|s| s.child.as_ref().map(Child::id)).collect::<Vec<_>>());
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
                say!("jamu: {} belum selesai setelah {budget} detik; dimatikan paksa", s.name);
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

// ---------------------------------------------------------------- perintah

fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::Laravel => "Laravel",
        Kind::WordPress => "WordPress",
        Kind::Php => "PHP",
    }
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
            say!("jamu: peringatan: {} tidak bisa ditulis oleh {who}; unggahan, cache, atau log situs akan gagal", d.display());
        }
    }
}

fn serve(o: Opts) -> Result<(), String> {
    os::init();
    let project = detect(&o.dir);
    let dev = o.domain.is_none() && !o.behind_proxy;
    let caddy = find_bin("JAMU_CADDY", &["caddy"])?;
    // JAMU_PHP_CGI di Linux memaksa jalur php-cgi, supaya jalur Windows bisa diuji di Linux.
    let cgi = cfg!(windows) || env::var_os("JAMU_PHP_CGI").is_some();
    let php_bin = if cgi {
        find_bin("JAMU_PHP_CGI", &["php-cgi", "php-cgi8.5", "php-cgi8.4", "php-cgi8.3", "php-cgi8.2"])?
    } else {
        find_bin("JAMU_PHP_FPM", &["php-fpm", "php-fpm8.5", "php-fpm8.4", "php-fpm8.3", "php-fpm8.2"])?
    };

    let free = |p: u16| port_free(p, dev);
    let port = match (&o.domain, o.port) {
        (Some(_), _) => {
            for p in [80, 443] {
                if !free(p) {
                    return Err(format!("Port {p} sedang dipakai{}. Mode domain butuh port 80 dan 443.", os::port_holder(p)));
                }
            }
            443
        }
        (None, None) if o.behind_proxy && free(8080) => 8080,
        (None, None) if o.behind_proxy => return Err(format!("Port 8080 sedang dipakai{}.", os::port_holder(8080))),
        (None, Some(p)) if free(p) => p,
        (None, Some(p)) => return Err(format!("Port {p} sedang dipakai{}.", os::port_holder(p))),
        (None, None) => {
            let p = (8080..8180).find(|p| free(*p)).ok_or("Tidak ada port bebas di 8080-8179.")?;
            if p != 8080 {
                say!("jamu: port 8080 sedang dipakai{}; memakai {p}", os::port_holder(8080));
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
    let write = |p: &Path, text: String| fs::write(p, text).map_err(|e| format!("tidak bisa menulis {}: {e}", p.display()));
    let php = if cgi {
        let ini = state.join("php.ini");
        write(&ini, cgi_ini(&php_bin, dev))?;
        Php::Cgi { bin: php_bin, ini, port: free_port()? }
    } else {
        // Panjang path soket Unix dibatasi kernel (108 byte).
        let mut sock = state.join("php-fpm.sock");
        if sock.as_os_str().len() > 100 {
            sock = PathBuf::from(format!("/tmp/jamu-{:08x}.sock", fnv1a(state.as_os_str().as_encoded_bytes())));
        }
        let _ = fs::remove_file(&sock);
        let conf = state.join("php-fpm.conf");
        write(&conf, fpm_conf(&sock, dev))?;
        Php::Fpm { bin: php_bin, conf, sock }
    };
    let plan = Plan { caddy, caddyfile: state.join("Caddyfile"), php };
    write(&plan.caddyfile, caddyfile(&project, &o, port, &state, &plan.php))?;

    let (php_bin, how) = match &plan.php {
        Php::Fpm { bin, .. } => (bin, format!("php-fpm ondemand, maksimal {} pekerja", workers())),
        Php::Cgi { bin, .. } => (bin, format!("php-cgi, {} pekerja", cgi_workers())),
    };
    let php_version = Command::new(php_bin)
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
        "jamu: PHP {php_version}{}, {how}",
        project.php_wanted.as_ref().map(|w| format!(" (composer.json meminta {w})")).unwrap_or_default()
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

/// Untuk HEALTHCHECK container: keluar 0 hanya bila Caddy dan php-fpm sama-sama menjawab.
fn health(args: &[String]) -> Result<(), String> {
    let port: u16 = match args {
        [] => 8080,
        [flag, n] if flag == "--port" => n.parse().map_err(|_| "--port harus angka 1-65535")?,
        _ => return Err(USAGE.into()),
    };
    let fail = |e: std::io::Error| format!("port {port} tidak menjawab: {e}");
    let mut conn = TcpStream::connect_timeout(&([127, 0, 0, 1], port).into(), Duration::from_secs(2)).map_err(fail)?;
    conn.set_read_timeout(Some(Duration::from_secs(2))).map_err(fail)?;
    std::io::Write::write_all(&mut conn, format!("GET {PING_PATH} HTTP/1.0\r\nHost: localhost\r\n\r\n").as_bytes()).map_err(fail)?;
    let mut reply = String::new();
    conn.read_to_string(&mut reply).map_err(fail)?;
    if reply.split_whitespace().nth(1) == Some("200") && reply.trim_end().ends_with("pong") {
        return Ok(());
    }
    Err(format!("php-fpm tidak menjawab ping: {}", reply.lines().next().unwrap_or("jawaban kosong")))
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("serve") => parse_opts(&args[1..]).and_then(serve),
        Some("health") => health(&args[1..]),
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

    fn fpm() -> Php {
        Php::Fpm { bin: "php-fpm".into(), conf: "/s/php-fpm.conf".into(), sock: "/s/php-fpm.sock".into() }
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
        let o = Opts { dir: dir.clone(), port: None, domain: None, email: None, acme_ca: None, acme_ca_root: None, behind_proxy: false };
        let text = caddyfile(&detect(&dir), &o, 8080, Path::new("/s"), &fpm());
        assert!(text.contains("bind 127.0.0.1"));
        assert!(text.contains("auto_https off"));
        assert!(text.contains("respond @hidden 404"));
        assert!(text.contains("respond @uploads_php 403"));
    }

    #[test]
    fn caddyfile_domain_tidak_mematikan_https() {
        let dir = project(&["index.php"]);
        let o = Opts { dir: dir.clone(), port: None, domain: Some("situs.test".into()), email: None, acme_ca: None, acme_ca_root: None, behind_proxy: false };
        let text = caddyfile(&detect(&dir), &o, 443, Path::new("/s"), &fpm());
        assert!(text.contains("situs.test {"));
        assert!(!text.contains("auto_https off"));
        assert!(!text.contains("@uploads_php"));
    }

    #[test]
    fn caddyfile_di_belakang_proxy_terbuka_dan_mengenali_https_terusan() {
        let dir = project(&["index.php"]);
        let o = Opts { dir: dir.clone(), port: None, domain: None, email: None, acme_ca: None, acme_ca_root: None, behind_proxy: true };
        let text = caddyfile(&detect(&dir), &o, 8080, Path::new("/s"), &fpm());
        assert!(!text.contains("bind 127.0.0.1"));
        assert!(text.contains("auto_https off"));
        assert!(text.contains("trusted_proxies static private_ranges"));
        assert!(text.contains("env HTTPS on"));
    }

    #[test]
    fn caddyfile_php_cgi_memakai_port_loopback_tanpa_ping() {
        let dir = project(&["index.php"]);
        let o = Opts { dir: dir.clone(), port: None, domain: None, email: None, acme_ca: None, acme_ca_root: None, behind_proxy: false };
        let php = Php::Cgi { bin: "php-cgi".into(), ini: "/s/php.ini".into(), port: 50123 };
        let text = caddyfile(&detect(&dir), &o, 8080, Path::new("/s"), &php);
        assert!(text.contains("php_fastcgi 127.0.0.1:50123 {"));
        assert!(!text.contains("@ping"));
        assert!(!text.contains("unix/"));
    }

    #[test]
    fn windows_ini_hanya_menyalakan_ekstensi_yang_ada() {
        let dir = project(&["ext/php_exif.dll", "ext/php_mbstring.dll", "ext/php_opcache.dll", "ext/php_tidak_dikenal.dll", "extras/ssl/cacert.pem"]);
        let text = windows_ini(&dir);
        let at = |needle: &str| text.find(needle).unwrap_or_else(|| panic!("{needle} tidak ada di:\n{text}"));
        assert!(at("extension = mbstring\n") < at("extension = exif\n"));
        at("zend_extension = opcache\n");
        at("curl.cainfo = ");
        assert!(!text.contains("gd"));
        assert!(!text.contains("tidak_dikenal"));
        assert!(!text.contains('\\'));
    }
}
