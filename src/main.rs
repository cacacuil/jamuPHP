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
    pub domain: Option<String>,
    pub email: Option<String>,
    pub acme_ca: Option<String>,
    pub acme_ca_root: Option<String>,
    pub behind_proxy: bool,
}

pub const USAGE: &str = "\
Pemakaian:
  jamu serve [DIR] [--port N]
  jamu serve [DIR] --domain NAMA [--email ALAMAT] [--acme-ca URL] [--acme-ca-root FILE]
  jamu serve [DIR] --behind-proxy [--port N]
  jamu php [ARGUMEN...]
  jamu composer [ARGUMEN...]
  jamu health [--port N]
  jamu detect [DIR]

Tanpa opsi: HTTP di 127.0.0.1, port 8080 atau port bebas berikutnya.
Dengan --domain: port 80 dan 443, sertifikat diterbitkan dan diperbarui otomatis.
Dengan --behind-proxy: HTTP di semua antarmuka, port 8080, untuk container di belakang
Traefik atau proxy lain yang mengurus HTTPS.
`jamu php` dan `jamu composer` menjalankan PHP yang sama dengan yang melayani situs, di
folder tempat perintah itu diketik.";

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
