//! Isi konfigurasi yang dibangkitkan: Caddyfile, php-fpm.conf, dan php.ini untuk php-cgi.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::thread;

use crate::Opts;
use crate::detect::{Kind, Project};
use crate::os;
use crate::supervise::LogLine;

/// Cara PHP dijalankan.
pub enum Php {
    /// Linux: php-fpm lewat soket Unix. Pekerja dibuat saat ada permintaan.
    Fpm {
        bin: PathBuf,
        conf: PathBuf,
        sock: PathBuf,
    },
    /// Windows, yang tidak punya php-fpm: php-cgi sebagai server FastCGI di port loopback.
    /// Dengan PHP_FCGI_CHILDREN ia sendiri yang membuat pekerja, membagi satu soket ke
    /// semuanya, dan mengganti pekerja yang keluar. Supervisor cukup mengawasi induknya.
    Cgi {
        bin: PathBuf,
        ini: PathBuf,
        port: u16,
    },
}

/// Perintah yang dijalankan shell di folder project: pekerja atau tugas terjadwal.
pub struct Task {
    pub name: String,
    pub run: String,
    pub log: LogLine,
}

pub struct Plan {
    pub caddy: PathBuf,
    pub caddyfile: PathBuf,
    pub php: Php,
    /// Folder project: tempat pekerja dan tugas terjadwal dijalankan.
    pub dir: PathBuf,
    /// Lingkungan untuk pekerja dan tugas terjadwal, supaya `php` di sana adalah PHP project.
    pub env: Vec<(OsString, OsString)>,
    /// Proses latar dari `jamu.toml`, diawasi seperti Caddy dan PHP.
    pub workers: Vec<Task>,
    /// Dijalankan di setiap pergantian menit.
    pub schedule: Option<Task>,
}

/// Dijawab php-fpm sendiri ("pong"), tanpa menjalankan kode situs. Caddy hanya meneruskannya
/// untuk permintaan dari loopback, jadi `jamu health` menguji Caddy dan php-fpm sekaligus.
pub const PING_PATH: &str = "/-/jamu/ping";

/// Bawaan opcache memeriksa perubahan file paling cepat tiap 2 detik. Di mode dev itu terasa
/// seperti "sudah disimpan tapi halamannya belum berubah", jadi diperiksa di setiap permintaan.
const DEV_REVALIDATE: u8 = 0;

pub fn workers() -> usize {
    // Dua kali jumlah core, minimal 4 supaya permintaan WordPress ke dirinya sendiri tidak macet.
    (thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2)
        * 2)
    .max(4)
}

/// Pekerja php-cgi selalu hidup semua (tidak ada ondemand) dan masing-masing memakai
/// sekitar 27 MB, jadi jumlahnya mengikuti jumlah core dengan batas 4 sampai 8.
pub fn cgi_workers() -> usize {
    thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .clamp(4, 8)
}

/// Pengaturan PHP yang ditetapkan jamu, sama di dev dan production kecuali soal menampilkan
/// galat, lalu ditimpa `[ini]` dari jamu.toml.
fn php_settings(dev: bool, user: &[(String, String)]) -> Vec<(String, String)> {
    let mut all: Vec<(String, String)> = [
        ("memory_limit", "256M"),
        ("upload_max_filesize", "128M"),
        ("post_max_size", "128M"),
        ("display_errors", if dev { "On" } else { "Off" }),
        ("log_errors", "On"),
    ]
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .into();
    if dev {
        all.push(("opcache.revalidate_freq".into(), DEV_REVALIDATE.to_string()));
    }
    for (k, v) in user {
        match all.iter_mut().find(|(name, _)| name == k) {
            Some(slot) => slot.1 = v.clone(),
            None => all.push((k.clone(), v.clone())),
        }
    }
    all
}

pub fn fpm_conf(sock: &Path, dev: bool, user: &[(String, String)]) -> String {
    let mut s = String::new();
    s += "[global]\nerror_log = /proc/self/fd/2\ndaemonize = no\n\n[www]\n";
    if os::is_root() {
        s += "user = root\ngroup = root\n";
    }
    s += &format!("listen = {}\nlisten.mode = 0600\n", sock.display());
    s += &format!(
        "pm = ondemand\npm.max_children = {}\npm.process_idle_timeout = 10s\n",
        workers()
    );
    s += "catch_workers_output = yes\ndecorate_workers_output = no\nclear_env = no\n";
    s += &format!("ping.path = {PING_PATH}\n");
    for (k, v) in php_settings(dev, user) {
        s += &format!("php_admin_value[{k}] = {v}\n");
    }
    s
}

/// Ekstensi dari zip resmi PHP untuk Windows yang dinyalakan bila filenya ada.
/// Urutan penting: exif butuh mbstring dimuat lebih dulu.
const WINDOWS_EXTENSIONS: &[&str] = &[
    "bz2",
    "curl",
    "fileinfo",
    "ftp",
    "gd",
    "gettext",
    "gmp",
    "intl",
    "mbstring",
    "exif",
    "mysqli",
    "openssl",
    "pdo_mysql",
    "pdo_pgsql",
    "pdo_sqlite",
    "pgsql",
    "soap",
    "sockets",
    "sodium",
    "sqlite3",
    "xsl",
    "zip",
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
pub fn cgi_ini(bin: &Path, dev: bool, user: &[(String, String)]) -> String {
    let mut s = String::new();
    for (k, v) in php_settings(dev, user) {
        s += &format!("{k} = {v}\n");
    }
    if cfg!(windows) {
        s += &windows_ini(bin.parent().unwrap_or(Path::new(".")));
    }
    s
}

/// php.ini tambahan untuk PHP di baris perintah (pekerja, penjadwal, `jamu php`): `[ini]`
/// dari jamu.toml, dan di Windows juga ekstensi dan daftar CA. Batas untuk web (memori,
/// ukuran unggahan) tidak ikut: Composer dan pekerja antrean punya kebutuhan sendiri.
pub fn cli_ini(bin: &Path, user: &[(String, String)]) -> String {
    let mut s = String::new();
    for (k, v) in user {
        s += &format!("{k} = {v}\n");
    }
    if cfg!(windows) {
        s += &windows_ini(bin.parent().unwrap_or(Path::new(".")));
    }
    s
}

/// Content-Type berkas statis menurut akhirannya, sama di semua OS.
const MIME_TYPES: &[(&str, &str)] = &[
    ("text/html; charset=utf-8", "html htm"),
    ("text/css; charset=utf-8", "css"),
    ("text/javascript; charset=utf-8", "js mjs"),
    ("application/json", "json map"),
    ("application/manifest+json", "webmanifest"),
    ("text/xml; charset=utf-8", "xml"),
    ("text/plain; charset=utf-8", "txt"),
    ("text/csv; charset=utf-8", "csv"),
    ("image/svg+xml", "svg"),
    ("image/png", "png"),
    ("image/jpeg", "jpg jpeg"),
    ("image/gif", "gif"),
    ("image/webp", "webp"),
    ("image/avif", "avif"),
    ("image/x-icon", "ico"),
    ("image/bmp", "bmp"),
    ("font/woff2", "woff2"),
    ("font/woff", "woff"),
    ("font/ttf", "ttf"),
    ("font/otf", "otf"),
    ("application/vnd.ms-fontobject", "eot"),
    ("application/pdf", "pdf"),
    ("application/wasm", "wasm"),
    ("video/mp4", "mp4 m4v"),
    ("video/webm", "webm"),
    ("audio/mpeg", "mp3"),
    ("audio/mp4", "m4a"),
    ("audio/ogg", "ogg"),
    ("audio/wav", "wav"),
    ("application/zip", "zip"),
    ("application/gzip", "gz"),
];

/// Path dalam tanda kutip untuk Caddyfile. Di dalam kutip Caddy hanya mengenal `\"`;
/// garis miring terbalik lain dibiarkan apa adanya. Path Windows ditulis dengan garis miring
/// biasa, yang diterima Caddy maupun PHP.
fn q(p: &Path) -> String {
    let s = p.display().to_string();
    let s = if cfg!(windows) {
        s.replace('\\', "/")
    } else {
        s
    };
    format!("\"{}\"", s.replace('"', "\\\""))
}

pub fn caddyfile(p: &Project, o: &Opts, port: u16, state: &Path, php: &Php) -> String {
    let mut s = String::from("{\n\tadmin off\n\tpersist_config off\n");
    s += &format!("\tstorage file_system {}\n", q(&state.join("caddy")));
    s += "\tlog {\n\t\tlevel WARN\n\t}\n";
    if o.domain.is_none() {
        // Tanpa TLS hanya HTTP/1.1 yang berlaku; menyebutnya terang-terangan membungkam
        // peringatan Caddy soal HTTP/2 dan HTTP/3 di setiap start.
        s += "\tservers {\n\t\tprotocols h1\n";
        if o.behind_proxy {
            // Alamat klien diambil dari X-Forwarded-For hanya bila pengirimnya proxy di jaringan
            // privat. Strict: dibaca dari kanan, jadi alamat yang diselipkan pengunjung di depan
            // daftar tidak pernah dipercaya.
            s += "\t\ttrusted_proxies static private_ranges\n\t\ttrusted_proxies_strict\n";
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
    // Urutan penanganan ditulis terang-terangan di dalam `route`, tidak diserahkan ke urutan
    // bawaan Caddy: tolak dulu, lalu PHP, lalu berkas statis.
    let mut route = String::new();
    // Dotfile (.env, .git, .htaccess) tidak pernah disajikan, kecuali .well-known.
    s += "\t@hidden {\n\t\tpath_regexp (^|/)\\.\n\t\tnot path /.well-known/*\n\t}\n";
    route += "\t\trespond @hidden 404\n";
    // jamu.toml ikut berada di document root bila root-nya folder project itu sendiri.
    s += "\t@jamu_toml path_regexp (?i)^/jamu\\.toml[.\\x20]*$\n";
    route += "\t\trespond @jamu_toml 404\n";
    if p.kind == Kind::WordPress {
        // Windows mengabaikan titik dan spasi di ujung nama file, jadi `x.php.` adalah `x.php`.
        // Spasi ditulis \x20 karena spasi biasa memotong token di Caddyfile.
        s += "\t@uploads_php path_regexp (?i)^/wp-content/uploads/.*\\.php[.\\x20]*(/|$)\n";
        route += "\t\trespond @uploads_php 403\n";
    }
    if p.kind == Kind::Laravel {
        // Hanya public/index.php yang dijalankan. Berkas .php lain di public, misalnya yang
        // ditaruh penyusup, diteruskan ke Laravel seperti URL biasa dan berakhir 404.
        s += "\t@other_php {\n\t\tpath_regexp (?i)\\.php[.\\x20]*(/|$)\n\t\tnot path /index.php /index.php/*\n\t}\n\trewrite @other_php /index.php\n";
    }
    let upstream = match php {
        Php::Fpm { sock, .. } => q(Path::new(&format!("unix/{}", sock.display()))),
        Php::Cgi { port, .. } => format!("127.0.0.1:{port}"),
    };
    if let Php::Fpm { .. } = php {
        s += &format!("\t@ping {{\n\t\tpath {PING_PATH}\n\t\tremote_ip 127.0.0.1 ::1\n\t}}\n");
        // php-fpm mengenali ping dari SCRIPT_NAME; tanpa split_path Caddy mengirimnya kosong.
        route += &format!(
            "\t\treverse_proxy @ping {upstream} {{\n\t\t\ttransport fastcgi {{\n\t\t\t\tenv SCRIPT_NAME {PING_PATH}\n\t\t\t\tenv SCRIPT_FILENAME {PING_PATH}\n\t\t\t}}\n\t\t}}\n"
        );
    }
    // flush_interval -1: keluaran PHP diteruskan saat itu juga, tidak ditampung dulu.
    // Tanpa ini respons yang dialirkan (SSE, unduhan yang dibangkitkan) baru tiba setelah selesai.
    let mut opts = String::from("\t\t\tflush_interval -1\n");
    if let Php::Cgi { .. } = php {
        // php-cgi mengirim catatan galatnya lewat FastCGI, dan Caddy membuangnya kecuali
        // diminta. php-fpm menulisnya ke log sendiri, yang sudah diteruskan jamu.
        opts += "\t\t\tcapture_stderr\n";
    }
    if p.kind == Kind::Laravel {
        // Bawaan Caddy juga mencoba {path}/index.php, yang akan menjalankan index.php di subfolder.
        opts += "\t\t\ttry_files {path} index.php\n";
    }
    if o.behind_proxy {
        // HTTPS berakhir di proxy. Tanpa HTTPS=on, WordPress mengira situsnya HTTP lalu
        // berputar-putar dalam redirect. Hanya dipercaya bila pengirimnya di jaringan privat.
        // REMOTE_ADDR diisi alamat pengunjung, bukan alamat proxy.
        s += "\t@fwd_https {\n\t\theader X-Forwarded-Proto https\n\t\tremote_ip private_ranges\n\t}\n";
        route += &format!(
            "\t\tphp_fastcgi @fwd_https {upstream} {{\n{opts}\t\t\tenv HTTPS on\n\t\t\tenv REMOTE_ADDR {{client_ip}}\n\t\t}}\n"
        );
        route += &format!(
            "\t\tphp_fastcgi {upstream} {{\n{opts}\t\t\tenv REMOTE_ADDR {{client_ip}}\n\t\t}}\n"
        );
    } else {
        route += &format!("\t\tphp_fastcgi {upstream} {{\n{opts}\t\t}}\n");
    }
    // Yang sampai di sini bukan urusan PHP, jadi Content-Type di bawah hanya mengenai berkas
    // statis. Caddy sendiri menebaknya dari tabel milik OS: di Windows tabel itu menyebut
    // .mjs sebagai text/plain, dan di image container tabelnya kosong.
    for (i, (mime, exts)) in MIME_TYPES.iter().enumerate() {
        let paths: Vec<String> = exts.split(' ').map(|e| format!("*.{e}")).collect();
        s += &format!("\t@mime{i} path {}\n", paths.join(" "));
        route += &format!("\t\theader @mime{i} Content-Type \"{mime}\"\n");
    }
    route += "\t\tfile_server\n";
    s += &format!("\tencode zstd gzip\n\troute {{\n{route}\t}}\n}}\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::detect;
    use crate::testutil::project;

    fn fpm() -> Php {
        Php::Fpm {
            bin: "php-fpm".into(),
            conf: "/s/php-fpm.conf".into(),
            sock: "/s/php-fpm.sock".into(),
        }
    }

    #[test]
    fn caddyfile_dev_hanya_di_loopback_dan_menolak_dotfile() {
        let dir = project(&["wp-load.php", "index.php"]);
        let o = Opts {
            dir: dir.clone(),
            ..Default::default()
        };
        let text = caddyfile(&detect(&dir), &o, 8080, Path::new("/s"), &fpm());
        assert!(text.contains("bind 127.0.0.1"));
        assert!(text.contains("auto_https off"));
        assert!(text.contains("respond @hidden 404"));
        assert!(text.contains("respond @uploads_php 403"));
    }

    #[test]
    fn caddyfile_domain_tidak_mematikan_https() {
        let dir = project(&["index.php"]);
        let o = Opts {
            dir: dir.clone(),
            domain: Some("situs.test".into()),
            ..Default::default()
        };
        let text = caddyfile(&detect(&dir), &o, 443, Path::new("/s"), &fpm());
        assert!(text.contains("situs.test {"));
        assert!(!text.contains("auto_https off"));
        assert!(!text.contains("@uploads_php"));
    }

    #[test]
    fn caddyfile_di_belakang_proxy_terbuka_dan_mengenali_https_terusan() {
        let dir = project(&["index.php"]);
        let o = Opts {
            dir: dir.clone(),
            behind_proxy: true,
            ..Default::default()
        };
        let text = caddyfile(&detect(&dir), &o, 8080, Path::new("/s"), &fpm());
        assert!(!text.contains("bind 127.0.0.1"));
        assert!(text.contains("auto_https off"));
        assert!(text.contains("trusted_proxies static private_ranges"));
        assert!(text.contains("trusted_proxies_strict"));
        assert!(text.contains("env HTTPS on"));
    }

    #[test]
    fn caddyfile_laravel_hanya_menjalankan_index_php() {
        let dir = project(&["artisan", "public/index.php"]);
        let o = Opts {
            dir: dir.clone(),
            ..Default::default()
        };
        let text = caddyfile(&detect(&dir), &o, 8080, Path::new("/s"), &fpm());
        assert!(text.contains("rewrite @other_php /index.php"));
        assert!(text.contains("try_files {path} index.php"));

        let dir = project(&["index.php"]);
        let text = caddyfile(&detect(&dir), &o, 8080, Path::new("/s"), &fpm());
        assert!(!text.contains("@other_php"));
        assert!(!text.contains("try_files"));
    }

    #[test]
    fn caddyfile_menetapkan_jenis_isi_sesudah_php_sebelum_berkas_statis() {
        let dir = project(&["index.php"]);
        let o = Opts {
            dir: dir.clone(),
            ..Default::default()
        };
        let text = caddyfile(&detect(&dir), &o, 8080, Path::new("/s"), &fpm());
        assert!(text.contains("path *.js *.mjs\n"));
        let at = |needle: &str| text.find(needle).expect(needle);
        // Urutan di dalam route: tolak, PHP, jenis isi, berkas statis.
        assert!(at("route {") < at("respond @hidden 404"));
        assert!(at("respond @hidden 404") < at("php_fastcgi"));
        assert!(at("php_fastcgi") < at("Content-Type \"text/javascript; charset=utf-8\""));
        assert!(at("Content-Type \"application/gzip\"") < at("file_server"));
    }

    #[test]
    fn caddyfile_php_cgi_memakai_port_loopback_tanpa_ping() {
        let dir = project(&["index.php"]);
        let o = Opts {
            dir: dir.clone(),
            ..Default::default()
        };
        let php = Php::Cgi {
            bin: "php-cgi".into(),
            ini: "/s/php.ini".into(),
            port: 50123,
        };
        let text = caddyfile(&detect(&dir), &o, 8080, Path::new("/s"), &php);
        assert!(text.contains("php_fastcgi 127.0.0.1:50123 {"));
        assert!(!text.contains("@ping"));
        assert!(!text.contains("unix/"));
    }

    #[test]
    fn windows_ini_hanya_menyalakan_ekstensi_yang_ada() {
        let dir = project(&[
            "ext/php_exif.dll",
            "ext/php_mbstring.dll",
            "ext/php_opcache.dll",
            "ext/php_tidak_dikenal.dll",
            "extras/ssl/cacert.pem",
        ]);
        let text = windows_ini(&dir);
        let at = |needle: &str| {
            text.find(needle)
                .unwrap_or_else(|| panic!("{needle} tidak ada di:\n{text}"))
        };
        assert!(at("extension = mbstring\n") < at("extension = exif\n"));
        at("zend_extension = opcache\n");
        at("curl.cainfo = ");
        assert!(!text.contains("gd"));
        assert!(!text.contains("tidak_dikenal"));
        assert!(!text.contains('\\'));
    }
}
