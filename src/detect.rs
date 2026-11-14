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

    Project {
        dir: dir.to_path_buf(),
        root,
        kind,
        php_wanted,
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
    fn subfolder_tanpa_index_diabaikan() {
        let dir = project(&["public/readme.txt", "index.php"]);
        assert_eq!(detect(&dir).root, dir);
    }
}
