//! `jamu check [DIR]`: memeriksa sebuah project sebelum dinyalakan atau dikirim ke server.
//!
//! Yang diperiksa adalah hal yang baru ketahuan setelah situs jalan: konfigurasi yang ditolak
//! Caddy atau PHP versi ini, ekstensi yang kurang, folder yang tidak bisa ditulis, dan folder
//! yang mengandalkan `.htaccess` untuk menolak akses. Keluar dengan kode 1 bila ada masalah.

use std::net::ToSocketAddrs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

use crate::cli::{check_version, php_cli, php_version};
use crate::config::{Limits, Php, body_limit, caddyfile};
use crate::detect::{Kind, kind_name};
use crate::mail;
use crate::paths::{find_bin, find_file, open_state};
use crate::serve::{find_php, laravel_hint, unwritable, write, write_php};
use crate::{Opts, settings};

#[derive(Default)]
struct Report {
    problems: usize,
}

impl Report {
    fn line(&self, label: &str, text: &str) {
        use std::io::Write;
        let _ = writeln!(std::io::stdout(), "  {label:<8} {text}");
    }
    fn ok(&self, text: &str) {
        self.line("ok", text);
    }
    fn note(&self, text: &str) {
        self.line("catatan", text);
    }
    fn problem(&mut self, text: &str) {
        self.line("MASALAH", text);
        self.problems += 1;
    }
}

/// Baris terakhir yang berisi dari keluaran sebuah program: di situlah pesan galatnya.
fn last_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("tanpa pesan")
        .trim()
        .to_string()
}

/// Larangan akses pertama di sebuah `.htaccess`: nomor barisnya, dan apakah ia berlaku
/// untuk seluruh folder (bukan hanya berkas tertentu di dalam blok `<Files>`).
fn htaccess_deny(text: &str) -> Option<(usize, bool)> {
    let mut depth = 0usize;
    let mut scoped = None;
    for (i, raw) in text.lines().enumerate() {
        let line = raw
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .to_ascii_lowercase();
        if line.starts_with("</files") {
            depth = depth.saturating_sub(1);
        } else if line.starts_with("<files") {
            depth += 1;
        } else if line == "deny from all" || line == "require all denied" {
            if depth == 0 {
                return Some((i + 1, true));
            }
            scoped.get_or_insert((i + 1, false));
        }
    }
    scoped
}

/// Semua `.htaccess` di bawah document root. Folder berawalan titik dilewati karena jamu
/// memang tidak menyajikannya.
fn htaccess_files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let name = e.file_name();
            let Ok(kind) = e.file_type() else { continue };
            if kind.is_dir() {
                if !name.to_string_lossy().starts_with('.') && name != "node_modules" {
                    stack.push(e.path());
                }
            } else if name == ".htaccess" {
                found.push(e.path());
            }
        }
    }
    found.sort();
    found
}

/// Nilai sebuah kunci di berkas `.env`, tanpa tanda kutip.
fn dotenv(text: &str, key: &str) -> Option<String> {
    text.lines().find_map(|l| {
        let l = l.trim();
        let l = l.strip_prefix("export ").unwrap_or(l);
        let v = l.strip_prefix(key)?.trim_start().strip_prefix('=')?.trim();
        Some(v.trim_matches(['"', '\'']).to_string())
    })
}

/// Variabel lingkungan menang atas `.env`, sama seperti di Laravel.
fn laravel_env(dir: &Path, key: &str) -> Option<String> {
    env::var(key).ok().or_else(|| {
        fs::read_to_string(dir.join(".env"))
            .ok()
            .and_then(|t| dotenv(&t, key))
    })
}

pub fn check(o: Opts) -> Result<(), String> {
    let (project, settings) = settings::load(&o.dir)?;
    let dev = o.domains.is_empty() && !o.behind_proxy;
    let rel = |p: &Path| {
        p.strip_prefix(&project.dir)
            .unwrap_or(p)
            .display()
            .to_string()
            .replace('\\', "/")
    };
    let mut r = Report::default();
    {
        use std::io::Write;
        let _ = writeln!(
            std::io::stdout(),
            "jamu check: {} di {}",
            kind_name(project.kind),
            project.dir.display()
        );
    }
    if project.dir.join(settings::FILE).is_file() {
        r.ok("jamu.toml terbaca");
    }

    // Program dan konfigurasi: yang diuji adalah berkas yang sama dengan yang ditulis `serve`,
    // di subfolder sendiri supaya tidak menimpa milik situs yang sedang jalan.
    let state = open_state(&project.dir)?;
    let dir = state.join("check");
    fs::create_dir_all(&dir).map_err(|e| format!("tidak bisa membuat {}: {e}", dir.display()))?;
    let mut ini = settings.ini();
    let mail_ok = match mail::add_sendmail(&mut ini, &state) {
        Ok(()) => true,
        Err(e) => {
            r.problem(&e);
            false
        }
    };
    let php = match find_php() {
        Ok(found) => {
            let version = php_version(&found.1);
            match check_version(&settings, &version) {
                Ok(()) => r.ok(&format!("PHP {version}")),
                Err(e) => r.problem(&e),
            }
            Some(write_php(
                &dir,
                &state,
                found,
                dev,
                &ini,
                settings.timeout(),
            )?)
        }
        Err(e) => {
            r.problem(&e);
            None
        }
    };
    if let Some(Php::Fpm { bin, conf, .. }) = &php {
        match Command::new(bin).arg("-t").arg("-y").arg(conf).output() {
            Ok(out) if out.status.success() => r.ok("konfigurasi php-fpm diterima"),
            Ok(out) => r.problem(&format!(
                "konfigurasi php-fpm ditolak: {}",
                last_line(&out.stderr)
            )),
            Err(e) => r.problem(&format!("php-fpm tidak bisa dijalankan: {e}")),
        }
    }
    match (find_bin("JAMU_CADDY", &["caddy"]), &php) {
        (Ok(caddy), Some(php)) => {
            let file = dir.join("Caddyfile");
            let port = o
                .port
                .unwrap_or(if o.domains.is_empty() { 8080 } else { 443 });
            let limits = Limits {
                body: body_limit(dev, &ini),
                timeout: settings.timeout(),
            };
            write(&file, caddyfile(&project, &o, port, &state, php, limits))?;
            let out = Command::new(&caddy)
                .args(["validate", "--adapter", "caddyfile", "--config"])
                .arg(&file)
                .output();
            match out {
                Ok(out) if out.status.success() => r.ok("konfigurasi Caddy diterima"),
                Ok(out) => r.problem(&format!(
                    "konfigurasi Caddy ditolak: {}",
                    last_line(&out.stderr)
                )),
                Err(e) => r.problem(&format!("Caddy tidak bisa dijalankan: {e}")),
            }
        }
        (Err(e), _) => r.problem(&e),
        (Ok(_), None) => {}
    }

    // Sertifikat hanya terbit bila CA bisa mencapai server ini lewat alamat di DNS. Alamat
    // server sendiri tidak selalu bisa diketahui dari dalam (NAT, container), jadi alamatnya
    // ditampilkan untuk dicocokkan; rekaman AAAA lama yang terlupa ikut terlihat di sini.
    for d in &o.domains {
        match (d.as_str(), 80).to_socket_addrs() {
            Ok(addrs) => {
                let mut ips: Vec<String> = addrs.map(|a| a.ip().to_string()).collect();
                ips.dedup();
                r.note(&format!(
                    "{d} mengarah ke {}; sertifikat terbit bila itu alamat server ini dan port 80 terbuka",
                    ips.join(", ")
                ));
            }
            Err(_) => r.problem(&format!(
                "{d} belum punya rekaman DNS (A atau AAAA): sertifikat tidak akan terbit"
            )),
        }
    }

    // Email: relay yang tidak menjawab baru ketahuan saat email pertama gagal, kecuali ditanya
    // sekarang. WordPress memakai mail() untuk reset sandi dan pemberitahuan pesanan.
    match mail::relay() {
        Ok(Some(relay)) if mail_ok && mail::wanted(&settings.ini()) == Ok(true) => {
            let at = format!("relay SMTP {}:{}", relay.host, relay.port);
            match relay.probe() {
                Ok(()) => r.ok(&format!("{at} menjawab")),
                Err(e) => r.problem(&format!("{at} tidak bisa dihubungi: {e}")),
            }
        }
        Ok(None) if !dev && project.kind == Kind::WordPress => r.note(
            "mail() tidak mengirim email: isi JAMU_SMTP_HOST untuk meneruskannya ke relay SMTP, atau pakai plugin SMTP",
        ),
        _ => {}
    }

    // Ekstensi dan versi PHP yang diminta paket: Composer sudah tahu cara memeriksanya.
    if project.dir.join("vendor/autoload.php").is_file() {
        let composer = find_file("JAMU_COMPOSER", &["composer.phar", "composer"]);
        match (composer, php_cli(&project.dir, &settings)) {
            (Ok(composer), Ok(cli)) => {
                let out = Command::new(&cli.bin)
                    .arg(composer)
                    .args(["check-platform-reqs", "--no-ansi", "--no-interaction"])
                    .current_dir(&project.dir)
                    .envs(cli.env.iter().map(|(k, v)| (k, v)))
                    .output();
                match out {
                    Ok(out) if out.status.success() => r.ok("kebutuhan paket Composer terpenuhi"),
                    Ok(out) => {
                        let text = String::from_utf8_lossy(&out.stdout);
                        let mut any = false;
                        for l in text
                            .lines()
                            .filter(|l| l.contains("missing") || l.contains("failed"))
                        {
                            any = true;
                            let l = l.split_whitespace().collect::<Vec<_>>().join(" ");
                            r.problem(&format!("kebutuhan paket Composer: {l}"));
                        }
                        if !any {
                            r.problem(&format!(
                                "Composer tidak bisa memeriksa kebutuhan paket: {}",
                                last_line(&out.stderr)
                            ));
                        }
                    }
                    Err(e) => r.problem(&format!("PHP tidak bisa dijalankan: {e}")),
                }
            }
            _ => r.note("Composer tidak ditemukan; ekstensi yang diminta paket tidak diperiksa"),
        }
    }

    match project.kind {
        Kind::Laravel => {
            if !project.dir.join("vendor/autoload.php").is_file() {
                r.problem("folder vendor belum ada: jalankan `jamu composer install`");
            }
            if laravel_env(&project.dir, "APP_KEY").is_none_or(|v| v.is_empty()) {
                r.problem("APP_KEY belum diisi: jalankan `jamu php artisan key:generate`");
            }
            let debug = laravel_env(&project.dir, "APP_DEBUG")
                .is_some_and(|v| matches!(v.to_ascii_lowercase().as_str(), "true" | "1" | "on"));
            if debug && !dev {
                r.problem(
                    "APP_DEBUG menyala: di server, halaman galat memperlihatkan isi konfigurasi ke pengunjung",
                );
            }
            if let Some(hint) = laravel_hint(&project).filter(|_| !dev) {
                r.note(hint);
            }
        }
        Kind::WordPress => {
            let config = ["wp-config.php", "../wp-config.php"]
                .iter()
                .any(|f| project.root.join(f).is_file());
            if !config {
                r.note("wp-config.php belum ada: situs akan menampilkan halaman pemasangan");
            }
        }
        Kind::Php => {}
    }
    for (d, who) in unwritable(&project) {
        r.problem(&format!(
            "{} tidak bisa ditulis oleh {who}: unggahan, cache, atau log situs akan gagal",
            rel(&d)
        ));
    }

    // jamu tidak membaca .htaccess. Untuk aturan tulis ulang itu tidak apa-apa, tetapi folder
    // yang ditutup lewat .htaccess menjadi terbuka, dan plugin yang menulisnya tidak tahu.
    for file in htaccess_files(&project.root) {
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        match htaccess_deny(&text) {
            Some((_, true)) => r.problem(&format!(
                "{} menutup folder itu, tetapi jamu tidak membaca .htaccess: isinya bisa diunduh siapa saja",
                rel(&file)
            )),
            Some((line, false)) => r.note(&format!(
                "{} baris {line} menolak akses ke berkas tertentu; jamu tidak membaca .htaccess, jadi larangan itu tidak berlaku",
                rel(&file)
            )),
            None => {}
        }
    }

    match r.problems {
        0 => {
            use std::io::Write;
            let _ = writeln!(std::io::stdout(), "Tidak ada masalah.");
            Ok(())
        }
        n => Err(format!("{n} masalah ditemukan.")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn larangan_htaccess_dibedakan_antara_folder_dan_berkas() {
        assert_eq!(htaccess_deny("Deny from all\n"), Some((1, true)));
        assert_eq!(
            htaccess_deny(
                "# komentar\n<IfModule mod_authz_core.c>\n  Require all  denied\n</IfModule>\n"
            ),
            Some((3, true))
        );
        // Hanya berkas tertentu: bukan seluruh folder.
        assert_eq!(
            htaccess_deny("<FilesMatch \"\\.php$\">\nDeny from all\n</FilesMatch>\n"),
            Some((2, false))
        );
        // Larangan folder menang walau ada larangan berkas lebih dulu.
        assert_eq!(
            htaccess_deny("<Files a.txt>\ndeny from all\n</Files>\nDENY FROM ALL\n"),
            Some((4, true))
        );
        // Komentar dan aturan tulis ulang bukan larangan.
        assert_eq!(
            htaccess_deny("# Deny from all\nRewriteEngine On\nRewriteRule . /index.php [L]\n"),
            None
        );
    }

    #[test]
    fn nilai_env_dibaca_tanpa_kutip() {
        let text = "APP_NAME=Toko\nAPP_KEY=\"base64:abc\"\nexport APP_DEBUG = true\nAPP_KEYS=x\n";
        assert_eq!(dotenv(text, "APP_KEY").as_deref(), Some("base64:abc"));
        assert_eq!(dotenv(text, "APP_DEBUG").as_deref(), Some("true"));
        assert_eq!(dotenv(text, "APP_URL"), None);
        assert_eq!(dotenv("APP_KEY=\n", "APP_KEY").as_deref(), Some(""));
    }
}
