//! Tempat mencari program (Caddy, PHP, Composer) dan folder state milik sebuah project.

use std::path::{Path, PathBuf};
use std::{env, fs};

use crate::os;

/// Folder tempat jamu berada dan subfolder `php` di sampingnya: yang dibawa bersama jamu.
fn bundled_dirs() -> Vec<PathBuf> {
    let beside = env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf));
    beside
        .iter()
        .flat_map(|d| [d.clone(), d.join("php")])
        .collect()
}

/// Mencari program dengan salah satu nama. Urutannya: variabel lingkungan, yang dibawa
/// bersama jamu, PATH, lalu folder tambahan khas OS.
pub fn find_bin(env_name: &str, names: &[&str]) -> Result<PathBuf, String> {
    let names: Vec<String> = names
        .iter()
        .map(|n| format!("{n}{}", env::consts::EXE_SUFFIX))
        .collect();
    find_file(
        env_name,
        &names.iter().map(String::as_str).collect::<Vec<_>>(),
    )
}

/// Seperti `find_bin`, untuk nama file apa adanya (tanpa akhiran .exe di Windows).
pub fn find_file(env_name: &str, names: &[&str]) -> Result<PathBuf, String> {
    if let Some(p) = env::var_os(env_name).filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(p));
    }
    let path = env::var_os("PATH").unwrap_or_default();
    for dir in bundled_dirs()
        .into_iter()
        .chain(env::split_paths(&path))
        .chain(os::BIN_DIRS.iter().map(PathBuf::from))
    {
        for name in names {
            let p = dir.join(name);
            if p.is_file() {
                return Ok(p);
            }
        }
    }
    Err(format!(
        "{} tidak ditemukan. Pasang, atau tunjuk lewat variabel {env_name}.",
        names[0]
    ))
}

pub fn fnv1a(bytes: &[u8]) -> u32 {
    bytes.iter().fold(0x811c9dc5u32, |h, b| {
        (h ^ *b as u32).wrapping_mul(0x01000193)
    })
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

/// Folder state project, dibuat bila belum ada. Letaknya di luar project, supaya tidak ikut
/// tersaji dan tidak mengotori repo.
pub fn open_state(project: &Path) -> Result<PathBuf, String> {
    let mut state = state_dir(&os::state_base(), project);
    if let Err(first) = fs::create_dir_all(state.join("caddy")) {
        let wanted = state.clone();
        let fallback = os::fallback_state_base().map(|base| state_dir(&base, project));
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
    Ok(state)
}
