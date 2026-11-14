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
    /// Tugas terjadwal saja: boleh dijalankan lagi selagi yang sebelumnya belum selesai.
    pub overlap: bool,
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

/// Memori yang disisihkan untuk satu pekerja PHP saat menghitung jumlahnya. Terukur: pekerja
/// WordPress dan Laravel memakai 15-45 MB; angka ini sudah termasuk bagian Caddy dan opcache.
const WORKER_MEMORY: u64 = 64 << 20;

/// Dua kali jumlah core, tetapi tidak lebih dari yang muat di memori container. Minimal 4
/// supaya permintaan WordPress ke dirinya sendiri tidak macet.
fn workers_for(cores: usize, memory: Option<u64>) -> usize {
    let by_memory = memory.map_or(usize::MAX, |m| (m / WORKER_MEMORY) as usize);
    (cores * 2).min(by_memory).max(4)
}

pub fn workers() -> usize {
    let cores = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(2);
    workers_for(cores, os::memory_limit())
}

/// Ukuran gaya php.ini ("128M", "1G", "2048") dalam byte.
fn php_bytes(v: &str) -> Option<u64> {
    let v = v.trim();
    let unit = match v.chars().last()?.to_ascii_lowercase() {
        'k' => 1 << 10,
        'm' => 1 << 20,
        'g' => 1 << 30,
        _ => 1u64,
    };
    let digits = if unit == 1 { v } else { &v[..v.len() - 1] };
    digits.trim().parse::<u64>().ok()?.checked_mul(unit)
}

/// Batas ukuran body permintaan: `post_max_size` yang berlaku, supaya Caddy dan PHP menolak
/// di angka yang sama. `None` bila PHP sendiri tidak membatasi atau nilainya tidak terbaca.
pub fn body_limit(dev: bool, user: &[(String, String)]) -> Option<u64> {
    let all = php_settings(dev, user);
    let (_, v) = all.iter().find(|(k, _)| k == "post_max_size")?;
    php_bytes(v).filter(|n| *n > 0)
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

pub fn fpm_conf(sock: &Path, dev: bool, user: &[(String, String)], timeout: Option<u64>) -> String {
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
    // Pekerja diganti setelah 500 permintaan, supaya kebocoran memori tidak menumpuk.
    s += "pm.max_requests = 500\n";
    if let Some(t) = timeout {
        // Caddy menjawab 504 tepat di batasnya; dua detik kemudian pekerja yang masih
        // menggantung dihentikan, supaya bisa melayani permintaan lain.
        s += &format!("request_terminate_timeout = {}s\n", t + 2);
    }
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

/// Cookie yang menandai pengunjung yang dikenal WordPress: masuk log, pernah berkomentar,
/// atau membuka tulisan bersandi. Mereka tidak boleh menerima halaman cache.
const WP_KNOWN_VISITOR: &str =
    "(comment_author|wordpress_[a-f0-9]+|wp-postpass|wordpress_logged_in)";

/// Aturan berkas cache WP Super Cache: halaman yang sudah dibuat plugin disajikan Caddy tanpa
/// menyentuh PHP, sehingga tetap terjawab saat PHP kebanjiran. Tidak ada cache yang dibangun
/// di sini; plugin tetap yang membuat dan membuang berkasnya. Syaratnya sama dengan aturan
/// .htaccess buatan plugin. Mengembalikan pencocok (tingkat situs) dan isi `route`.
fn wp_cache_rule(o: &Opts) -> (String, String) {
    // Plugin menyimpan halaman HTTPS dan HTTP di berkas terpisah.
    let from_proxy = "header X-Forwarded-Proto https\n\t\tremote_ip private_ranges";
    let variants: Vec<(&str, &str, String)> = if o.behind_proxy {
        vec![
            (
                "wpsc_https",
                "index-https.html",
                format!("\t\t{from_proxy}\n"),
            ),
            (
                "wpsc_http",
                "index.html",
                format!(
                    "\t\tnot {{\n\t\t\t{}\n\t\t}}\n",
                    from_proxy.replace("\t\t", "\t\t\t")
                ),
            ),
        ]
    } else if !o.domains.is_empty() {
        vec![
            (
                "wpsc_https",
                "index-https.html",
                "\t\tprotocol https\n".into(),
            ),
            ("wpsc_http", "index.html", "\t\tprotocol http\n".into()),
        ]
    } else {
        vec![("wpsc_http", "index.html", String::new())]
    };
    // Bila folder plugin dihapus tanpa dinonaktifkan, berkas cache tertinggal dan tidak ada
    // lagi yang membuangnya. Karena itu aturan hanya berlaku selama berkas plugin masih ada.
    let mut matchers = String::from(
        "\t@wpsc_plugin file /wp-content/plugins/wp-super-cache/wp-cache-phase1.php\n",
    );
    let mut route = String::from("\t\troute @wpsc_plugin {\n");
    for (name, file, scheme) in variants {
        // Nama host ikut menjadi nama folder, jadi hanya bentuk yang lazim yang diterima.
        matchers += &format!(
            "\t@{name} {{\n\t\tmethod GET HEAD\n\t\tpath */\n\t\tnot path_regexp //\n\
             \t\texpression `{{http.request.uri.query}} == \"\" && {{http.request.host}}.matches(\"^[a-z0-9-]+(\\\\.[a-z0-9-]+)*$\")`\n\
             \t\tnot header_regexp Cookie {WP_KNOWN_VISITOR}\n{scheme}\
             \t\tfile /wp-content/cache/supercache/{{host}}{{path}}{file}\n\t}}\n"
        );
        route += &format!(
            "\t\t\theader @{name} Cache-Control \"max-age=3, must-revalidate\"\n\
             \t\t\theader @{name} Vary \"Accept-Encoding, Cookie\"\n\
             \t\t\trewrite @{name} {{file_match.relative}}\n"
        );
    }
    route += "\t\t}\n";
    (matchers, route)
}

/// Batas yang sama di Caddy dan PHP: ukuran body dan lama satu permintaan.
#[derive(Clone, Copy, Default)]
pub struct Limits {
    pub body: Option<u64>,
    pub timeout: Option<u64>,
}

pub fn caddyfile(
    p: &Project,
    o: &Opts,
    port: u16,
    state: &Path,
    php: &Php,
    limits: Limits,
) -> String {
    let mut s = String::from("{\n\tadmin off\n\tpersist_config off\n");
    s += &format!("\tstorage file_system {}\n", q(&state.join("caddy")));
    if o.domains.is_empty() {
        s += "\tlog {\n\t\tlevel WARN\n\t}\n";
    } else {
        // Terbit dan diperpanjangnya sertifikat adalah berita, walau Caddy mencatatnya sebagai
        // INFO. Supervisor meringkas baris-baris ini dan membuang sisanya.
        s += "\tlog {\n\t\tlevel WARN\n\t\texclude tls.obtain tls.renew\n\t}\n";
        s += "\tlog sertifikat {\n\t\tlevel INFO\n\t\tinclude tls.obtain tls.renew\n\t\tformat json\n\t}\n";
    }
    if o.domains.is_empty() {
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
    match o.domains.first() {
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

    match o.domains.first() {
        // Dev hanya mendengarkan di loopback: situs lokal tidak boleh terbuka ke jaringan.
        None if o.behind_proxy => s += &format!("http://:{port} {{\n"),
        None => s += &format!("http://:{port} {{\n\tbind 127.0.0.1\n"),
        Some(_) => s += &format!("{} {{\n", o.domains.join(", ")),
    }
    s += &format!("\troot * {}\n", q(&p.root));
    if let Some(bytes) = limits.body {
        s += &format!("\trequest_body {{\n\t\tmax_size {bytes}\n\t}}\n");
    }
    // Log akses, satu baris JSON per permintaan. jamu meringkasnya menjadi satu baris pendek.
    s += "\tlog {\n\t\toutput stderr\n\t\tformat json\n\t}\n";
    // Urutan penanganan ditulis terang-terangan di dalam `route`, tidak diserahkan ke urutan
    // bawaan Caddy: tolak dulu, lalu PHP, lalu berkas statis.
    let mut route = String::new();
    if p.wp_subdir_multisite {
        // Padanan aturan .htaccess yang dibuat WordPress untuk multisite subdirektori: awalan
        // nama situs (`/toko/wp-admin/...`) dibuang bila berkasnya tidak ada di path itu.
        // Ditaruh paling depan, supaya aturan tolak di bawah memeriksa path sesudah ditulis ulang.
        s += "\t@ms_admin path_regexp ^/[_0-9a-zA-Z-]+/wp-admin$\n";
        route += "\t\tredir @ms_admin {path}/ 301\n";
        for (name, pattern) in [
            ("ms_files", "(/wp-(content|admin|includes).*)"),
            ("ms_php", "(/.*\\.php)$"),
        ] {
            s += &format!(
                "\t@{name} {{\n\t\tpath_regexp {name} ^/[_0-9a-zA-Z-]+{pattern}\n\t\tnot file {{path}} {{path}}/\n\t}}\n"
            );
            route += &format!("\t\trewrite @{name} {{re.{name}.1}}\n");
        }
    }
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
        // Pemeriksaan kesehatan datang tiap beberapa detik; tidak perlu memenuhi log akses.
        route += "\t\tlog_skip @ping\n";
        // php-fpm mengenali ping dari SCRIPT_NAME; tanpa split_path Caddy mengirimnya kosong.
        route += &format!(
            "\t\treverse_proxy @ping {upstream} {{\n\t\t\ttransport fastcgi {{\n\t\t\t\tenv SCRIPT_NAME {PING_PATH}\n\t\t\t\tenv SCRIPT_FILENAME {PING_PATH}\n\t\t\t}}\n\t\t}}\n"
        );
    }
    if p.kind == Kind::WordPress {
        let (matchers, rule) = wp_cache_rule(o);
        s += &matchers;
        route += &rule;
    }
    // flush_interval -1: keluaran PHP diteruskan saat itu juga, tidak ditampung dulu.
    // Tanpa ini respons yang dialirkan (SSE, unduhan yang dibangkitkan) baru tiba setelah selesai.
    let mut opts = String::from("\t\t\tflush_interval -1\n");
    if let Some(t) = limits.timeout {
        // Dihitung sejak permintaan diteruskan ke PHP. Jawabannya 504, bukan 502, supaya
        // terbedakan dari PHP yang mati.
        opts += &format!("\t\t\tread_timeout {t}s\n");
    }
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
    // Satu tabel, dicari sekali per permintaan. Satu matcher dan satu `header` per jenis
    // (31 pasang) terukur memperlambat berkas statis sekitar 20%.
    s += "\tmap {file.ext} {jamu_mime} {\n";
    for (mime, exts) in MIME_TYPES {
        s += &format!("\t\t~(?i)^\\.({})$ \"{mime}\"\n", exts.replace(' ', "|"));
    }
    s += "\t\tdefault \"\"\n\t}\n\t@mime not vars {jamu_mime} \"\"\n";
    route += "\t\theader @mime Content-Type {jamu_mime}\n";
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
    fn jumlah_pekerja_mengikuti_core_dan_memori() {
        const MB: u64 = 1 << 20;
        assert_eq!(workers_for(4, None), 8);
        assert_eq!(workers_for(1, None), 4);
        // Container kecil di mesin besar: memori yang menentukan.
        assert_eq!(workers_for(32, Some(512 * MB)), 8);
        assert_eq!(workers_for(32, Some(1024 * MB)), 16);
        assert_eq!(workers_for(4, Some(256 * MB)), 4);
        // Tidak pernah di bawah 4, dan memori lapang tidak menambah di atas batas core.
        assert_eq!(workers_for(4, Some(128 * MB)), 4);
        assert_eq!(workers_for(4, Some(64 * 1024 * MB)), 8);
    }

    #[test]
    fn batas_body_mengikuti_post_max_size() {
        assert_eq!(php_bytes("128M"), Some(128 << 20));
        assert_eq!(php_bytes("1g"), Some(1 << 30));
        assert_eq!(php_bytes("64K"), Some(64 << 10));
        assert_eq!(php_bytes("2048"), Some(2048));
        assert_eq!(php_bytes("banyak"), None);
        assert_eq!(php_bytes(""), None);

        let ini = |v: &str| vec![("post_max_size".to_string(), v.to_string())];
        assert_eq!(body_limit(false, &[]), Some(128 << 20));
        assert_eq!(body_limit(false, &ini("300M")), Some(300 << 20));
        // PHP tanpa batas, atau nilai yang tidak terbaca: Caddy tidak menambah batas sendiri.
        assert_eq!(body_limit(false, &ini("0")), None);
        assert_eq!(body_limit(false, &ini("banyak")), None);

        let dir = project(&["index.php"]);
        let o = Opts {
            dir: dir.clone(),
            ..Default::default()
        };
        let text = |limits| caddyfile(&detect(&dir), &o, 8080, Path::new("/s"), &fpm(), limits);
        let with = text(Limits {
            body: Some(300 << 20),
            timeout: Some(300),
        });
        assert!(with.contains("max_size 314572800\n"));
        assert!(with.contains("read_timeout 300s\n"));
        let without = text(Limits::default());
        assert!(!without.contains("request_body"));
        assert!(!without.contains("read_timeout"));
    }

    #[test]
    fn php_fpm_menghentikan_permintaan_yang_melewati_batas_waktu() {
        let conf = fpm_conf(Path::new("/s/php-fpm.sock"), false, &[], Some(300));
        assert!(conf.contains("request_terminate_timeout = 302s\n"));
        assert!(conf.contains("pm.max_requests = 500\n"));
        let conf = fpm_conf(Path::new("/s/php-fpm.sock"), false, &[], None);
        assert!(!conf.contains("request_terminate_timeout"));
    }

    #[test]
    fn caddyfile_dev_hanya_di_loopback_dan_menolak_dotfile() {
        let dir = project(&["wp-load.php", "index.php"]);
        let o = Opts {
            dir: dir.clone(),
            ..Default::default()
        };
        let text = caddyfile(
            &detect(&dir),
            &o,
            8080,
            Path::new("/s"),
            &fpm(),
            Limits::default(),
        );
        assert!(text.contains("bind 127.0.0.1"));
        assert!(text.contains("auto_https off"));
        assert!(text.contains("respond @hidden 404"));
        assert!(text.contains("respond @uploads_php 403"));
    }

    #[test]
    fn caddyfile_wordpress_menyajikan_berkas_cache_plugin_sebelum_php() {
        let dir = project(&["wp-load.php", "index.php"]);
        let text = |o: Opts| {
            caddyfile(
                &detect(&dir),
                &o,
                8080,
                Path::new("/s"),
                &fpm(),
                Limits::default(),
            )
        };
        let opts = || Opts {
            dir: dir.clone(),
            ..Default::default()
        };

        // Dev: hanya HTTP, jadi hanya varian index.html.
        let dev = text(opts());
        assert!(dev.contains("{host}{path}index.html"));
        assert!(!dev.contains("index-https.html"));
        // Hanya selama berkas plugin ada, sesudah aturan tolak, sebelum PHP.
        let at = |needle: &str| dev.find(needle).expect(needle);
        assert!(at("respond @uploads_php 403") < at("route @wpsc_plugin {"));
        assert!(at("route @wpsc_plugin {") < at("php_fastcgi"));
        assert!(dev.contains("not header_regexp Cookie (comment_author|"));

        // Di belakang proxy: HTTPS dikenali dari proxy yang dipercaya.
        let proxy = text(Opts {
            behind_proxy: true,
            ..opts()
        });
        assert!(proxy.contains("{host}{path}index-https.html"));
        assert!(proxy.contains("{host}{path}index.html"));
        assert!(
            proxy.contains("@wpsc_https {") && proxy.contains("header X-Forwarded-Proto https")
        );

        // Mode domain: HTTPS dikenali dari koneksinya sendiri.
        let domain = text(Opts {
            domains: vec!["situs.test".into()],
            ..opts()
        });
        assert!(domain.contains("protocol https") && domain.contains("protocol http\n"));

        // Bukan WordPress: tidak ada aturan cache.
        let plain = project(&["index.php"]);
        let other = caddyfile(
            &detect(&plain),
            &opts(),
            8080,
            Path::new("/s"),
            &fpm(),
            Limits::default(),
        );
        assert!(!other.contains("wpsc"));
    }

    #[test]
    fn caddyfile_multisite_subdirektori_menulis_ulang_sebelum_aturan_tolak() {
        let dir = project(&["wp-load.php", "index.php"]);
        let o = Opts {
            dir: dir.clone(),
            ..Default::default()
        };
        let single = caddyfile(
            &detect(&dir),
            &o,
            8080,
            Path::new("/s"),
            &fpm(),
            Limits::default(),
        );
        assert!(!single.contains("@ms_"));

        std::fs::write(
            dir.join("wp-config.php"),
            "<?php\ndefine('MULTISITE', true);\n",
        )
        .unwrap();
        let text = caddyfile(
            &detect(&dir),
            &o,
            8080,
            Path::new("/s"),
            &fpm(),
            Limits::default(),
        );
        let at = |needle: &str| text.find(needle).expect(needle);
        assert!(at("route {") < at("rewrite @ms_files {re.ms_files.1}"));
        assert!(at("rewrite @ms_php {re.ms_php.1}") < at("respond @hidden 404"));
        assert!(at("rewrite @ms_php {re.ms_php.1}") < at("respond @uploads_php 403"));
    }

    #[test]
    fn caddyfile_mencatat_akses_kecuali_pemeriksaan_kesehatan() {
        let dir = project(&["index.php"]);
        let o = Opts {
            dir: dir.clone(),
            ..Default::default()
        };
        let text = caddyfile(
            &detect(&dir),
            &o,
            8080,
            Path::new("/s"),
            &fpm(),
            Limits::default(),
        );
        assert!(text.contains("output stderr"));
        assert!(text.contains("log_skip @ping"));
    }

    #[test]
    fn caddyfile_domain_tidak_mematikan_https() {
        let dir = project(&["index.php"]);
        let o = Opts {
            dir: dir.clone(),
            domains: vec!["situs.test".into()],
            ..Default::default()
        };
        let text = caddyfile(
            &detect(&dir),
            &o,
            443,
            Path::new("/s"),
            &fpm(),
            Limits::default(),
        );
        assert!(text.contains("situs.test {"));
        assert!(!text.contains("auto_https off"));
        assert!(!text.contains("@uploads_php"));
        // Kabar sertifikat dicatat terpisah, dan tidak dobel di log utama.
        assert!(text.contains("exclude tls.obtain tls.renew"));
        assert!(text.contains("log sertifikat {"));

        // Beberapa nama: satu situs, satu blok.
        let o = Opts {
            dir: dir.clone(),
            domains: vec!["situs.test".into(), "www.situs.test".into()],
            ..Default::default()
        };
        let text = caddyfile(
            &detect(&dir),
            &o,
            443,
            Path::new("/s"),
            &fpm(),
            Limits::default(),
        );
        assert!(text.contains("\nsitus.test, www.situs.test {\n"));

        // Di luar mode domain tidak ada yang perlu dicatat soal sertifikat.
        let o = Opts {
            dir: dir.clone(),
            ..Default::default()
        };
        let text = caddyfile(
            &detect(&dir),
            &o,
            8080,
            Path::new("/s"),
            &fpm(),
            Limits::default(),
        );
        assert!(!text.contains("sertifikat"));
    }

    #[test]
    fn caddyfile_di_belakang_proxy_terbuka_dan_mengenali_https_terusan() {
        let dir = project(&["index.php"]);
        let o = Opts {
            dir: dir.clone(),
            behind_proxy: true,
            ..Default::default()
        };
        let text = caddyfile(
            &detect(&dir),
            &o,
            8080,
            Path::new("/s"),
            &fpm(),
            Limits::default(),
        );
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
        let text = caddyfile(
            &detect(&dir),
            &o,
            8080,
            Path::new("/s"),
            &fpm(),
            Limits::default(),
        );
        assert!(text.contains("rewrite @other_php /index.php"));
        assert!(text.contains("try_files {path} index.php"));

        let dir = project(&["index.php"]);
        let text = caddyfile(
            &detect(&dir),
            &o,
            8080,
            Path::new("/s"),
            &fpm(),
            Limits::default(),
        );
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
        let text = caddyfile(
            &detect(&dir),
            &o,
            8080,
            Path::new("/s"),
            &fpm(),
            Limits::default(),
        );
        // Satu tabel dan satu `header`: akhiran dicari sekali per permintaan, huruf besar
        // atau kecil.
        assert!(text.contains("\tmap {file.ext} {jamu_mime} {\n"));
        assert!(text.contains("\t\t~(?i)^\\.(js|mjs)$ \"text/javascript; charset=utf-8\"\n"));
        assert!(text.contains("\t\t~(?i)^\\.(gz)$ \"application/gzip\"\n"));
        assert_eq!(text.matches("header @mime").count(), 1);
        let at = |needle: &str| text.find(needle).expect(needle);
        // Urutan di dalam route: tolak, PHP, jenis isi, berkas statis.
        assert!(at("route {") < at("respond @hidden 404"));
        assert!(at("respond @hidden 404") < at("php_fastcgi"));
        assert!(at("php_fastcgi") < at("header @mime Content-Type {jamu_mime}"));
        assert!(at("header @mime Content-Type {jamu_mime}") < at("file_server"));
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
        let text = caddyfile(
            &detect(&dir),
            &o,
            8080,
            Path::new("/s"),
            &php,
            Limits::default(),
        );
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
