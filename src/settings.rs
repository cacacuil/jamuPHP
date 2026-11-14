//! `jamu.toml`: file opsional di folder project yang menimpa hasil deteksi.
//!
//! Semua baris opsional dan file ini boleh tidak ada. Kunci yang tidak dikenal ditolak,
//! supaya salah ketik tidak diam-diam diabaikan.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::detect::{Project, detect};
use crate::os;

pub const FILE: &str = "jamu.toml";

#[derive(Deserialize, Default, Debug)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// Versi PHP yang diminta, misalnya "8.4". `serve` menolak jalan dengan versi lain.
    pub php: Option<String>,
    /// Document root, relatif terhadap folder project.
    pub root: Option<String>,
    /// Penjadwal Laravel (`schedule:run` tiap menit). Bawaan: menyala kecuali di mode dev.
    pub scheduler: Option<bool>,
    /// Batas waktu satu permintaan PHP, dalam detik. 0 berarti tanpa batas.
    timeout: Option<u64>,
    /// Pengaturan php.ini untuk project ini.
    #[serde(default)]
    ini: BTreeMap<String, toml::Value>,
    /// Proses latar yang ikut dinyalakan dan diawasi, misalnya pekerja antrean.
    #[serde(default, rename = "worker")]
    pub workers: Vec<Worker>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Worker {
    pub name: String,
    pub run: String,
}

/// Nama yang dipakai jamu sendiri sebagai awalan baris log.
const RESERVED: &[&str] = &["jamu", "caddy", "php-fpm", "php-cgi", "schedule"];

/// Batas waktu bawaan satu permintaan PHP. Cukup longgar untuk pembaruan plugin dan impor,
/// cukup ketat supaya permintaan yang menggantung tidak menyandera pekerja selamanya.
const TIMEOUT: u64 = 300;

impl Settings {
    /// Batas waktu satu permintaan PHP dalam detik; `None` berarti tanpa batas.
    pub fn timeout(&self) -> Option<u64> {
        Some(self.timeout.unwrap_or(TIMEOUT)).filter(|t| *t > 0)
    }

    /// Pengaturan php.ini sebagai pasangan nama dan nilai, siap ditulis.
    pub fn ini(&self) -> Vec<(String, String)> {
        self.ini
            .iter()
            .map(|(k, v)| {
                let v = match v {
                    toml::Value::String(s) => s.clone(),
                    toml::Value::Boolean(b) => if *b { "On" } else { "Off" }.to_string(),
                    toml::Value::Integer(n) => n.to_string(),
                    toml::Value::Float(n) => n.to_string(),
                    // Jenis lain sudah ditolak `check`.
                    _ => String::new(),
                };
                (k.clone(), v)
            })
            .collect()
    }

    fn check(&self) -> Result<(), String> {
        if let Some(v) = &self.php {
            let ok = v.split_once('.').is_some_and(|(a, b)| {
                !a.is_empty()
                    && !b.is_empty()
                    && (a.to_owned() + b).bytes().all(|c| c.is_ascii_digit())
            });
            if !ok {
                return Err(format!(
                    "php = \"{v}\": tulis versi sebagai dua angka, misalnya \"8.4\""
                ));
            }
        }
        for (k, v) in &self.ini {
            let name_ok = !k.is_empty()
                && k.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'.');
            if !name_ok {
                return Err(format!(
                    "[ini] {k}: nama pengaturan hanya boleh huruf, angka, titik, dan garis bawah"
                ));
            }
            let value_ok = match v {
                toml::Value::String(s) => !s.chars().any(char::is_control),
                toml::Value::Integer(_) | toml::Value::Float(_) | toml::Value::Boolean(_) => true,
                _ => false,
            };
            if !value_ok {
                return Err(format!(
                    "[ini] {k}: nilainya harus teks satu baris, angka, atau true/false"
                ));
            }
        }
        for (i, w) in self.workers.iter().enumerate() {
            let name_ok = !w.name.is_empty()
                && w.name.len() <= 32
                && w.name.bytes().all(|c| {
                    c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-' || c == b'_'
                });
            if !name_ok {
                return Err(format!(
                    "[[worker]] name = \"{}\": pakai huruf kecil, angka, - atau _, paling banyak 32 karakter",
                    w.name
                ));
            }
            if RESERVED.contains(&w.name.as_str()) {
                return Err(format!(
                    "[[worker]] name = \"{}\": nama itu dipakai jamu sendiri",
                    w.name
                ));
            }
            if self.workers[..i].iter().any(|other| other.name == w.name) {
                return Err(format!("[[worker]] name = \"{}\" dipakai dua kali", w.name));
            }
            if w.run.trim().is_empty() || w.run.chars().any(char::is_control) {
                return Err(format!(
                    "[[worker]] \"{}\": run harus berisi satu baris perintah",
                    w.name
                ));
            }
        }
        Ok(())
    }
}

fn parse(text: &str) -> Result<Settings, String> {
    let s: Settings = toml::from_str(text).map_err(|e| e.to_string())?;
    s.check()?;
    Ok(s)
}

/// Mengenali project di `dir`, lalu menimpanya dengan `jamu.toml` bila ada.
pub fn load(dir: &Path) -> Result<(Project, Settings), String> {
    let mut project = detect(dir);
    let file = dir.join(FILE);
    let text = match fs::read_to_string(&file) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok((project, Settings::default()));
        }
        Err(e) => return Err(format!("{} tidak bisa dibaca: {e}", file.display())),
    };
    let settings = parse(&text)
        .map_err(|e| format!("{} tidak bisa dipakai: {}", file.display(), e.trim_end()))?;
    if let Some(root) = &settings.root {
        let wanted = dir.join(root);
        let real = fs::canonicalize(&wanted).map(os::tidy).map_err(|e| {
            format!(
                "{}: root = \"{root}\" tidak bisa dibuka ({e})",
                file.display()
            )
        })?;
        if !real.is_dir() || !real.starts_with(dir) {
            return Err(format!(
                "{}: root = \"{root}\" harus berupa folder di dalam project",
                file.display()
            ));
        }
        project.root = real;
    }
    Ok((project, settings))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::project;

    #[test]
    fn semua_kunci_terbaca() {
        let s = parse(
            r#"
            php = "8.4"
            root = "web"
            scheduler = false
            timeout = 600

            [ini]
            memory_limit = "512M"
            max_execution_time = 120
            display_errors = true
            "date.timezone" = "Asia/Jakarta"

            [[worker]]
            name = "queue"
            run = "php artisan queue:work --tries=3"
            "#,
        )
        .unwrap();
        assert_eq!(s.php.as_deref(), Some("8.4"));
        assert_eq!(s.root.as_deref(), Some("web"));
        assert_eq!(s.scheduler, Some(false));
        assert_eq!(s.timeout(), Some(600));
        assert_eq!(Settings::default().timeout(), Some(300));
        assert_eq!(parse("timeout = 0").unwrap().timeout(), None);
        assert!(parse("timeout = -5").is_err());
        let ini = s.ini();
        let get = |k: &str| {
            ini.iter()
                .find(|(name, _)| name == k)
                .map(|(_, v)| v.as_str())
        };
        assert_eq!(get("memory_limit"), Some("512M"));
        assert_eq!(get("max_execution_time"), Some("120"));
        assert_eq!(get("display_errors"), Some("On"));
        assert_eq!(get("date.timezone"), Some("Asia/Jakarta"));
        assert_eq!(s.workers[0].name, "queue");
    }

    #[test]
    fn file_kosong_sah() {
        let s = parse("").unwrap();
        assert!(s.php.is_none() && s.workers.is_empty() && s.ini().is_empty());
    }

    #[test]
    fn yang_salah_ditolak_dengan_alasan() {
        let err = |text: &str| parse(text).unwrap_err();
        assert!(err("rot = \"public\"").contains("rot"));
        assert!(err("php = \"8\"").contains("8.4"));
        assert!(err("php = \"^8.2\"").contains("8.4"));
        assert!(err("[ini]\n\"memory limit\" = \"1G\"").contains("memory limit"));
        assert!(err("[ini]\nmemory_limit = \"1G\\nextension=x\"").contains("memory_limit"));
        assert!(err("[ini]\nx = [1]").contains("x"));
        assert!(err("[[worker]]\nname = \"Antrean\"\nrun = \"x\"").contains("Antrean"));
        assert!(err("[[worker]]\nname = \"caddy\"\nrun = \"x\"").contains("caddy"));
        assert!(err("[[worker]]\nname = \"a\"\nrun = \" \"").contains("run"));
        assert!(
            err("[[worker]]\nname = \"a\"\nrun = \"x\"\n[[worker]]\nname = \"a\"\nrun = \"y\"")
                .contains("dua kali")
        );
        assert!(err("[[worker]]\nname = \"a\"").contains("run"));
        assert!(err("[[worker]]\nname = \"a\"\nrun = \"x\"\nrestart = false").contains("restart"));
    }

    #[test]
    fn root_dari_file_menimpa_deteksi_dan_tidak_boleh_keluar_project() {
        let dir = fs::canonicalize(project(&["index.php", "web/index.php", "jamu.toml"]))
            .map(os::tidy)
            .unwrap();
        fs::write(dir.join(FILE), "root = \"web\"").unwrap();
        let (p, _) = load(&dir).unwrap();
        assert_eq!(p.root, dir.join("web"));

        fs::write(dir.join(FILE), "root = \"..\"").unwrap();
        assert!(load(&dir).unwrap_err().contains("di dalam project"));
        fs::write(dir.join(FILE), "root = \"tidak-ada\"").unwrap();
        assert!(load(&dir).unwrap_err().contains("tidak-ada"));
        fs::write(dir.join(FILE), "root = \"index.php\"").unwrap();
        assert!(load(&dir).unwrap_err().contains("folder"));
    }

    #[test]
    fn tanpa_file_hasil_deteksi_dipakai_apa_adanya() {
        let dir = project(&["public/index.php", "artisan"]);
        let (p, s) = load(&dir).unwrap();
        assert_eq!(p.root, dir.join("public"));
        assert!(s.root.is_none());
    }
}
