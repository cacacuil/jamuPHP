//! Mengenali project dari isi foldernya: jenis, document root, dan versi PHP yang diminta.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Kind {
    Laravel,
    WordPress,
    Php,
}

#[derive(Debug)]
pub struct Project {
    pub dir: PathBuf,
    pub root: PathBuf,
    pub kind: Kind,
    /// Batasan versi PHP dari composer.json, misalnya "^8.2".
    pub php_wanted: Option<String>,
    /// WordPress multisite dengan situs di subfolder (`/toko/`, `/blog/`). Butuh aturan
    /// tulis-ulang tambahan; multisite subdomain tidak.
    pub wp_subdir_multisite: bool,
}

/// Nilai `define('NAMA', true|false)` di wp-config.php. Baris komentar dilewati.
fn wp_define(config: &str, name: &str) -> Option<bool> {
    config
        .lines()
        .map(str::trim_start)
        .filter(|l| !(l.starts_with("//") || l.starts_with('#') || l.starts_with('*')))
        .find_map(|line| {
            let line: String = line
                .chars()
                .filter(|c| !c.is_whitespace())
                .map(|c| if c == '"' { '\'' } else { c })
                .collect();
            let (_, value) = line.split_once(&format!("define('{name}',"))?;
            let value = value.to_ascii_lowercase();
            if value.starts_with("true") || value.starts_with('1') {
                Some(true)
            } else if value.starts_with("false") || value.starts_with('0') {
                Some(false)
            } else {
                None
            }
        })
}

fn wp_subdir_multisite(root: &Path) -> bool {
    // WordPress juga menerima wp-config.php satu folder di atas.
    let config = [root.join("wp-config.php"), root.join("../wp-config.php")]
        .iter()
        .find_map(|f| fs::read_to_string(f).ok())
        .unwrap_or_default();
    wp_define(&config, "MULTISITE") == Some(true)
        && wp_define(&config, "SUBDOMAIN_INSTALL") != Some(true)
}

pub fn detect(dir: &Path) -> Project {
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

    let wp_subdir_multisite = kind == Kind::WordPress && wp_subdir_multisite(&root);

    Project {
        dir: dir.to_path_buf(),
        root,
        kind,
        php_wanted,
        wp_subdir_multisite,
    }
}

pub fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::Laravel => "Laravel",
        Kind::WordPress => "WordPress",
        Kind::Php => "PHP",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::project;

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
    fn multisite_subdirektori_dikenali_dari_wp_config() {
        let with = |config: &str| {
            let dir = project(&["wp-load.php", "index.php"]);
            fs::write(dir.join("wp-config.php"), config).unwrap();
            detect(&dir).wp_subdir_multisite
        };
        assert!(with(
            "<?php\ndefine( 'MULTISITE', true );\ndefine( 'SUBDOMAIN_INSTALL', false );\n"
        ));
        assert!(with("<?php\ndefine(\"MULTISITE\",TRUE);\n"));
        // Subdomain: aturan biasa sudah cukup.
        assert!(!with(
            "<?php\ndefine( 'MULTISITE', true );\ndefine( 'SUBDOMAIN_INSTALL', true );\n"
        ));
        // Baru diizinkan, belum dinyalakan.
        assert!(!with("<?php\ndefine( 'WP_ALLOW_MULTISITE', true );\n"));
        assert!(!with("<?php\n// define( 'MULTISITE', true );\n"));
        assert!(!with("<?php\ndefine( 'MULTISITE', false );\n"));
        assert!(!with("<?php\n"));
    }

    #[test]
    fn subfolder_tanpa_index_diabaikan() {
        let dir = project(&["public/readme.txt", "index.php"]);
        assert_eq!(detect(&dir).root, dir);
    }
}
