//! PHP di baris perintah untuk sebuah project: dipakai pekerja, penjadwal, dan perintah
//! `jamu php` serta `jamu composer`. Tujuannya satu: `php` di terminal sama dengan PHP yang
//! melayani situs, dengan ekstensi dan pengaturan yang sama.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

use crate::config::cli_ini;
use crate::mail;
use crate::os;
use crate::paths::{find_bin, find_file, open_state};
use crate::settings::{self, Settings};

pub struct PhpCli {
    pub bin: PathBuf,
    /// Variabel lingkungan yang membuat `php` di proses anak menjadi PHP ini.
    pub env: Vec<(OsString, OsString)>,
}

/// Versi PHP sebuah binary, misalnya "8.4.27". "?" bila tidak bisa ditanya.
pub fn php_version(bin: &Path) -> String {
    Command::new(bin)
        .arg("-v")
        .output()
        .ok()
        .and_then(|out| {
            String::from_utf8_lossy(&out.stdout)
                .split_whitespace()
                .nth(1)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "?".into())
}

/// `php = "8.4"` di jamu.toml adalah janji: project ini tidak dijalankan dengan versi lain.
pub fn check_version(settings: &Settings, version: &str) -> Result<(), String> {
    match &settings.php {
        Some(want) if !version.starts_with(&format!("{want}.")) => Err(format!(
            "Project ini meminta PHP {want} ({}), tetapi yang terpasang PHP {version}.",
            settings::FILE
        )),
        _ => Ok(()),
    }
}

pub fn php_cli(dir: &Path, settings: &Settings) -> Result<PhpCli, String> {
    let bin = find_bin("JAMU_PHP", &["php", "php8.5", "php8.4", "php8.3", "php8.2"])?;
    let mut vars = Vec::new();
    // Folder PHP di depan PATH: `php artisan ...` di perintah pekerja memakai PHP ini.
    if let Some(bin_dir) = bin.parent() {
        let rest = env::var_os("PATH").unwrap_or_default();
        let path = env::join_paths(
            [bin_dir.to_path_buf()]
                .into_iter()
                .chain(env::split_paths(&rest)),
        )
        .map_err(|e| format!("PATH tidak bisa disusun: {e}"))?;
        vars.push(("PATH".into(), path));
    }
    let mut user = settings.ini();
    let mail = mail::wanted(&user)?;
    // Folder state baru dibuka bila memang ada yang perlu ditulis.
    if mail || !user.is_empty() || cfg!(windows) {
        let state = open_state(dir)?;
        mail::add_sendmail(&mut user, &state)?;
        let ini = cli_ini(&bin, &user);
        let ini_dir = state.join("cli");
        fs::create_dir_all(&ini_dir)
            .map_err(|e| format!("tidak bisa membuat {}: {e}", ini_dir.display()))?;
        // Zip resmi PHP untuk Windows tidak membawa php.ini, jadi file ini menjadi php.ini-nya.
        // Di tempat lain php.ini milik sistem tetap dipakai dan file ini dibaca sesudahnya.
        let (file, var) = if cfg!(windows) {
            ("php.ini", "PHPRC")
        } else {
            ("jamu.ini", "PHP_INI_SCAN_DIR")
        };
        let path = ini_dir.join(file);
        fs::write(&path, ini).map_err(|e| format!("tidak bisa menulis {}: {e}", path.display()))?;
        let value = if cfg!(windows) {
            ini_dir.into_os_string()
        } else {
            // Bagian kosong di depan titik dua berarti "folder bawaan PHP juga dibaca".
            let mut v = env::var_os("PHP_INI_SCAN_DIR").unwrap_or_default();
            v.push(":");
            v.push(&ini_dir);
            v
        };
        vars.push((var.into(), value));
    }
    Ok(PhpCli { bin, env: vars })
}

/// Menjalankan PHP project di folder ini dengan argumen yang diberikan, dan keluar dengan
/// kode keluarnya.
fn forward(before: &[OsString], args: &[String]) -> Result<(), String> {
    let dir = env::current_dir()
        .and_then(fs::canonicalize)
        .map(os::tidy)
        .map_err(|e| format!("folder kerja tidak bisa dibuka: {e}"))?;
    let (_, settings) = settings::load(&dir)?;
    let cli = php_cli(&dir, &settings)?;
    if settings.php.is_some() {
        check_version(&settings, &php_version(&cli.bin))?;
    }
    let mut cmd = Command::new(&cli.bin);
    cmd.args(before)
        .args(args)
        .envs(cli.env.iter().map(|(k, v)| (k, v)));
    Err(os::replace_process(cmd))
}

pub fn php(args: &[String]) -> Result<(), String> {
    forward(&[], args)
}

pub fn composer(args: &[String]) -> Result<(), String> {
    let composer = find_file("JAMU_COMPOSER", &["composer.phar", "composer"]).map_err(|_| {
        "Composer tidak ditemukan. Taruh composer.phar di samping jamu, pasang Composer, \
         atau tunjuk lewat variabel JAMU_COMPOSER."
            .to_string()
    })?;
    forward(&[composer.into_os_string()], args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versi_php_dicocokkan_dengan_dua_angka_pertama() {
        let with = |php: &str| {
            let mut s = Settings::default();
            s.php = Some(php.into());
            s
        };
        assert!(check_version(&with("8.4"), "8.4.27").is_ok());
        assert!(check_version(&Settings::default(), "8.3.6").is_ok());
        let err = check_version(&with("8.4"), "8.3.6").unwrap_err();
        assert!(err.contains("8.4") && err.contains("8.3.6"));
        // "8.4" tidak boleh cocok dengan 8.41.
        assert!(check_version(&with("8.4"), "8.41.0").is_err());
    }
}
