//! `jamu serve [DIR]` mengenali project PHP, menulis konfigurasi Caddy dan PHP ke folder
//! state di luar project, lalu menyalakan dan mengawasi keduanya sampai dihentikan.
//!
//! Di Linux PHP dijalankan php-fpm lewat soket Unix. Di Windows tidak ada php-fpm, jadi
//! yang dijalankan php-cgi sebagai server FastCGI di satu port loopback. Semua yang
//! berbeda antar-OS ada di modul `os`.

use std::path::PathBuf;
use std::process::ExitCode;
use std::{env, fs};

/// Menulis ke stderr tanpa panik bila stderr sudah ditutup (misalnya keluaran disalurkan ke `head`).
macro_rules! say {
    ($($arg:tt)*) => {{
        use std::io::Write;
        let _ = writeln!(std::io::stderr(), $($arg)*);
    }};
}

mod check;
mod cli;
mod config;
mod detect;
#[cfg_attr(unix, path = "os_unix.rs")]
#[cfg_attr(windows, path = "os_windows.rs")]
mod os;
mod paths;
mod serve;
mod settings;
mod supervise;

use detect::kind_name;

#[derive(Default)]
pub struct Opts {
    pub dir: PathBuf,
    pub port: Option<u16>,
    /// Mode domain bila tidak kosong. Semua nama dilayani situs yang sama.
    pub domains: Vec<String>,
    pub email: Option<String>,
    pub acme_ca: Option<String>,
    pub acme_ca_root: Option<String>,
    pub behind_proxy: bool,
}

pub const USAGE: &str = "\
Pemakaian:
  jamu serve [DIR] [--port N]
  jamu serve [DIR] --domain NAMA [--domain NAMA...] [--email ALAMAT]
                   [--acme-ca URL] [--acme-ca-root FILE]
  jamu serve [DIR] --behind-proxy [--port N]
  jamu check [DIR] [--domain NAMA... | --behind-proxy]
  jamu php [ARGUMEN...]
  jamu composer [ARGUMEN...]
  jamu health [--port N]
  jamu detect [DIR]

Tanpa opsi: HTTP di 127.0.0.1, port 8080 atau port bebas berikutnya.
Dengan --domain: port 80 dan 443, sertifikat diterbitkan dan diperbarui otomatis.
--domain boleh diulang, misalnya untuk nama dengan dan tanpa www.
Dengan --behind-proxy: HTTP di semua antarmuka, port 8080, untuk container di belakang
Traefik atau proxy lain yang mengurus HTTPS.
`jamu check` memeriksa project sebelum dinyalakan atau dikirim ke server, dengan opsi
yang sama seperti `serve`; keluar dengan kode 1 bila ada masalah.
`jamu php` dan `jamu composer` menjalankan PHP yang sama dengan yang melayani situs, di
folder tempat perintah itu diketik.";

/// Nama domain ditulis apa adanya ke konfigurasi Caddy, dan di hosting ia bisa berasal dari
/// isian pelanggan. Hanya huruf, angka, tanda hubung, dan titik yang diterima.
fn domain_name(name: &str) -> Result<String, String> {
    let name = name.to_ascii_lowercase();
    let label_ok = |l: &str| {
        !l.is_empty()
            && l.len() <= 63
            && !l.starts_with('-')
            && !l.ends_with('-')
            && l.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
    };
    if name.len() > 253 || !name.contains('.') || !name.split('.').all(label_ok) {
        return Err(format!(
            "--domain \"{name}\": tulis nama domain seperti situs.com, tanpa https:// dan tanpa garis miring"
        ));
    }
    Ok(name)
}

/// Nilai yang ditulis ke konfigurasi Caddy tidak boleh membawa spasi atau kurung kurawal.
fn one_word(name: &str, value: String) -> Result<String, String> {
    let bad = |c: char| c.is_whitespace() || c.is_control() || matches!(c, '{' | '}' | '"' | '`');
    if value.is_empty() || value.contains(bad) {
        return Err(format!(
            "{name}: nilainya tidak boleh kosong, berspasi, atau memuat kurung kurawal"
        ));
    }
    Ok(value)
}

fn parse_opts(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts {
        dir: PathBuf::from("."),
        ..Default::default()
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = |name: &str| it.next().cloned().ok_or(format!("{name} butuh nilai"));
        match a.as_str() {
            "--port" => {
                o.port = Some(
                    val("--port")?
                        .parse()
                        .map_err(|_| "--port harus angka 1-65535")?,
                )
            }
            "--domain" => o.domains.push(domain_name(&val("--domain")?)?),
            "--behind-proxy" => o.behind_proxy = true,
            "--email" => o.email = Some(one_word("--email", val("--email")?)?),
            "--acme-ca" => o.acme_ca = Some(one_word("--acme-ca", val("--acme-ca")?)?),
            "--acme-ca-root" => o.acme_ca_root = Some(val("--acme-ca-root")?),
            s if s.starts_with("--") => return Err(format!("opsi tidak dikenal: {s}")),
            s => o.dir = PathBuf::from(s),
        }
    }
    if o.behind_proxy && !o.domains.is_empty() {
        return Err("--behind-proxy dan --domain tidak bisa dipakai bersamaan: pilih siapa yang mengurus HTTPS".into());
    }
    o.dir = fs::canonicalize(&o.dir)
        .map(os::tidy)
        .map_err(|e| format!("folder {} tidak bisa dibuka: {e}", o.dir.display()))?;
    if !o.dir.is_dir() {
        return Err(format!("{} bukan folder", o.dir.display()));
    }
    Ok(o)
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("serve") => parse_opts(&args[1..]).and_then(serve::serve),
        Some("check") => parse_opts(&args[1..]).and_then(check::check),
        Some("health") => serve::health(&args[1..]),
        Some("php") => cli::php(&args[1..]),
        Some("composer") => cli::composer(&args[1..]),
        Some("detect") => parse_opts(&args[1..])
            .and_then(|o| settings::load(&o.dir))
            .map(|(p, s)| {
                println!("jenis: {}", kind_name(p.kind));
                println!("document root: {}", p.root.display());
                let wanted = s.php.as_deref().or(p.php_wanted.as_deref());
                println!("php diminta: {}", wanted.unwrap_or("-"));
                for w in &s.workers {
                    println!("pekerja {}: {}", w.name, w.run);
                }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nama_domain_yang_aneh_ditolak() {
        assert_eq!(domain_name("Situs.COM").unwrap(), "situs.com");
        assert_eq!(
            domain_name("www.toko-saya.co.id").unwrap(),
            "www.toko-saya.co.id"
        );
        for bad in [
            "",
            "localhost",
            "https://situs.com",
            "situs.com/",
            "situs.com {",
            "situs .com",
            "situs.com\nrespond 200",
            "*.situs.com",
            "-situs.com",
            "situs..com",
            ".situs.com",
        ] {
            assert!(domain_name(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn nilai_opsi_tidak_boleh_menyusup_ke_konfigurasi() {
        assert!(one_word("--email", "admin@situs.com".into()).is_ok());
        assert!(one_word("--acme-ca", "https://ca.test/dir".into()).is_ok());
        for bad in ["", "a b", "a\n}", "a{", "a\"b"] {
            assert!(one_word("--email", bad.into()).is_err(), "{bad}");
        }
    }
}

#[cfg(test)]
mod testutil {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::{env, fs};

    /// Folder project sementara berisi file-file kosong yang disebut.
    pub fn project(files: &[&str]) -> PathBuf {
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = env::temp_dir().join(format!(
            "jamu-test-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        for f in files {
            let p = dir.join(f);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(
                p,
                if f.ends_with("composer.json") {
                    r#"{"require":{"php":"^8.3"}}"#
                } else {
                    ""
                },
            )
            .unwrap();
        }
        dir
    }
}
