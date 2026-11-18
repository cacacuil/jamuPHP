//! Cache Laravel (`php artisan optimize`): konfigurasi, rute, event, dan view.
//!
//! Tanpa cache itu Laravel membaca ulang `.env` dan semua berkas config di tiap permintaan;
//! terukur pada aplikasi baru 584 req/dtk tanpa, 978 dengan. Di server jamu membangunnya
//! sendiri tiap kali dinyalakan, sebelum situs menerima permintaan: variabel lingkungan
//! container baru ada saat itu, dan tidak ada permintaan yang membaca berkas cache yang
//! sedang ditulis.

use std::path::Path;
use std::process::Command;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use std::{fs, io};

use crate::cli::PhpCli;
use crate::detect::{Kind, Project};
use crate::supervise::{Once, run_once};

/// Batas waktu `artisan optimize`. Aplikasi besar selesai dalam beberapa detik; yang
/// melewati ini biasanya menunggu basis data yang tidak menjawab, dan situs tidak boleh
/// ikut menunggu.
const BUDGET: Duration = Duration::from_secs(60);

/// Tempat `env()` tidak boleh dipanggil: sesudah konfigurasi di-cache Laravel tidak lagi
/// memuat `.env`, jadi di luar berkas config nilainya `null`.
const SCANNED: &[&str] = &["app", "routes", "resources/views"];

#[derive(Debug, PartialEq)]
pub enum Cache {
    /// Bukan Laravel, `vendor` belum ada, mode dev, atau dimatikan di `jamu.toml`.
    Leave,
    Build,
    /// Tidak dibangun karena aplikasinya memanggil `env()` di luar berkas config.
    Unsafe(String),
}

/// Yang dilakukan `serve` terhadap cache Laravel. `setting` adalah `optimize` di `jamu.toml`:
/// `true` memaksa, `false` mematikan, kosong berarti menyala di server bila aman.
pub fn cache(p: &Project, setting: Option<bool>, dev: bool) -> Cache {
    if p.kind != Kind::Laravel || !p.dir.join("vendor/autoload.php").is_file() {
        return Cache::Leave;
    }
    match setting {
        Some(true) => Cache::Build,
        Some(false) => Cache::Leave,
        None if dev => Cache::Leave,
        None => {
            let calls = env_calls(&p.dir);
            match calls.first() {
                None => Cache::Build,
                Some((file, line)) => {
                    let more = match calls.len() - 1 {
                        0 => String::new(),
                        n => format!(" (dan {n} tempat lain)"),
                    };
                    Cache::Unsafe(format!(
                        "{file} baris {line} memanggil env() di luar berkas config{more}; sesudah konfigurasi di-cache nilainya null. Pindahkan ke berkas config, atau paksa dengan optimize = true di jamu.toml"
                    ))
                }
            }
        }
    }
}

/// Cache yang tertinggal di laptop: Laravel berhenti membaca `.env`, dan itu membingungkan
/// justru di tempat `.env` paling sering diubah.
pub fn dev_note(p: &Project, setting: Option<bool>, dev: bool) -> Option<&'static str> {
    let stale = dev
        && p.kind == Kind::Laravel
        && setting != Some(true)
        && p.dir.join("bootstrap/cache/config.php").is_file();
    stale.then_some(
        "konfigurasi Laravel sedang di-cache (bootstrap/cache/config.php), jadi perubahan .env dan config tidak terbaca; hapus dengan `jamu php artisan optimize:clear`",
    )
}

/// Apakah baris ini memanggil fungsi `env()` milik Laravel. Yang bukan: `getenv(`,
/// `->env(`, `::env(`, `$env(`, direktif Blade `@env(`, dan baris komentar.
fn calls_env(line: &str) -> bool {
    let code = line.trim_start();
    let comment = code.starts_with("//")
        || code.starts_with('*')
        || code.starts_with("/*")
        || (code.starts_with('#') && !code.starts_with("#["));
    !comment
        && line.match_indices("env(").any(|(i, _)| {
            !line[..i]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '>' | ':' | '$' | '@'))
        })
}

fn scan(dir: &Path, rel: &str, found: &mut Vec<(String, usize)>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let name = e.file_name().to_string_lossy().into_owned();
        let rel = format!("{rel}/{name}");
        // Tautan simbolik tidak diikuti, supaya tidak berputar-putar.
        match e.file_type() {
            Ok(t) if t.is_dir() => scan(&e.path(), &rel, found),
            Ok(t) if t.is_file() && name.ends_with(".php") => {
                let Ok(bytes) = fs::read(e.path()) else {
                    continue;
                };
                let text = String::from_utf8_lossy(&bytes);
                found.extend(
                    text.lines()
                        .enumerate()
                        .filter(|(_, l)| calls_env(l))
                        .map(|(i, _)| (rel.clone(), i + 1)),
                );
            }
            _ => {}
        }
    }
}

/// Pemanggilan `env()` di kode aplikasi: berkas (relatif terhadap project) dan nomor baris.
/// Paket di `vendor` tidak diperiksa.
fn env_calls(dir: &Path) -> Vec<(String, usize)> {
    let mut found = Vec::new();
    for sub in SCANNED {
        scan(&dir.join(sub), sub, &mut found);
    }
    found
}

/// Baris-baris terakhir yang berisi dari keluaran artisan: di situlah pesan galatnya.
fn tail(path: &Path, n: usize) -> io::Result<Vec<String>> {
    let text = String::from_utf8_lossy(&fs::read(path)?).into_owned();
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    Ok(lines[lines.len().saturating_sub(n)..]
        .iter()
        .map(|l| l.trim_end().to_string())
        .collect())
}

/// Menjalankan `php artisan optimize` dan menunggunya. Gagal atau terlalu lama tidak
/// menghalangi situs menyala: situs berjalan dengan cache yang ada, dan sebabnya diberitakan.
pub fn optimize(dir: &Path, cli: &PhpCli, state: &Path, stop: &AtomicBool) {
    let out = state.join("optimize.log");
    let mut cmd = Command::new(&cli.bin);
    cmd.args(["artisan", "optimize", "--no-ansi", "--no-interaction"])
        .current_dir(dir)
        .envs(cli.env.iter().map(|(k, v)| (k, v)));
    let start = Instant::now();
    let why = match run_once(cmd, &out, BUDGET, stop) {
        Ok(Once::Done) => {
            let secs = format!("{:.1}", start.elapsed().as_secs_f64()).replace('.', ",");
            say!("jamu: cache Laravel dibangun (`php artisan optimize`, {secs} detik)");
            return;
        }
        Ok(Once::Stopped) => return,
        Ok(Once::Failed(status)) => format!("gagal ({status})"),
        Ok(Once::TimedOut) => format!(
            "belum selesai setelah {} detik dan dihentikan",
            BUDGET.as_secs()
        ),
        Err(e) => format!("tidak bisa dijalankan ({e})"),
    };
    say!(
        "jamu: peringatan: `php artisan optimize` {why}; situs dinyalakan tanpa cache baru. Langkah ini dimatikan dengan optimize = false di jamu.toml"
    );
    for line in tail(&out, 15).unwrap_or_default() {
        say!("[optimize] {line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings;
    use crate::testutil::project;

    #[test]
    fn hanya_panggilan_env_milik_laravel_yang_dihitung() {
        for ya in [
            "return env('KUNCI');",
            "$a = [env(\"A\"), 2];",
            "{{ env('APP_NAME') }}",
            "return \\env('KUNCI');",
            "if (!env('X')) {",
            "#[Atribut(env('X'))]",
        ] {
            assert!(calls_env(ya), "{ya}");
        }
        for bukan in [
            "$a = getenv('X');",
            "$b = $request->env('Y');",
            "$c = Lingkungan::env('Z');",
            "$d = $env('Z');",
            "@env('local')",
            "// env('KOMENTAR')",
            "   * env('DOKUMENTASI')",
            "# env('KOMENTAR')",
            "$e = my_env('X');",
            "app()->environment('local')",
        ] {
            assert!(!calls_env(bukan), "{bukan}");
        }
    }

    #[test]
    fn cache_dibangun_di_server_kecuali_dimatikan_atau_tidak_aman() {
        let dir = project(&[
            "artisan",
            "public/index.php",
            "vendor/autoload.php",
            "config/app.php",
            "app/Aman.php",
        ]);
        fs::write(dir.join("config/app.php"), "<?php return [env('A')];").unwrap();
        fs::write(dir.join("app/Aman.php"), "<?php $a = getenv('A');").unwrap();
        let (p, _) = settings::load(&dir).unwrap();
        assert_eq!(cache(&p, None, false), Cache::Build);
        assert_eq!(cache(&p, None, true), Cache::Leave);
        assert_eq!(cache(&p, Some(true), true), Cache::Build);
        assert_eq!(cache(&p, Some(false), false), Cache::Leave);

        // env() di kode aplikasi: tidak dibangun kecuali dipaksa.
        fs::create_dir_all(dir.join("resources/views/toko")).unwrap();
        fs::write(dir.join("app/Bayar.php"), "<?php\n\nreturn env('KUNCI');\n").unwrap();
        fs::write(
            dir.join("resources/views/toko/kasir.blade.php"),
            "{{ env('NAMA') }}",
        )
        .unwrap();
        let Cache::Unsafe(why) = cache(&p, None, false) else {
            panic!("seharusnya tidak aman");
        };
        assert!(
            why.starts_with("app/Bayar.php baris 3 memanggil env()"),
            "{why}"
        );
        assert!(why.contains("(dan 1 tempat lain)"), "{why}");
        assert_eq!(cache(&p, Some(true), false), Cache::Build);
        assert_eq!(cache(&p, Some(false), false), Cache::Leave);

        // Tanpa vendor artisan tidak bisa jalan; bukan Laravel bukan urusan modul ini.
        fs::remove_file(dir.join("vendor/autoload.php")).unwrap();
        assert_eq!(cache(&p, Some(true), false), Cache::Leave);
        let (polos, _) = settings::load(&project(&["index.php"])).unwrap();
        assert_eq!(cache(&polos, Some(true), false), Cache::Leave);
    }

    #[test]
    fn cache_yang_tertinggal_disebut_hanya_di_dev() {
        let dir = project(&["artisan", "public/index.php", "vendor/autoload.php"]);
        let (p, _) = settings::load(&dir).unwrap();
        assert!(dev_note(&p, None, true).is_none());
        fs::create_dir_all(dir.join("bootstrap/cache")).unwrap();
        fs::write(dir.join("bootstrap/cache/config.php"), "<?php return [];").unwrap();
        assert!(dev_note(&p, None, true).unwrap().contains("optimize:clear"));
        assert!(dev_note(&p, None, false).is_none());
        assert!(dev_note(&p, Some(true), true).is_none());
        assert!(dev_note(&p, Some(false), true).is_some());
    }

    #[test]
    fn ekor_keluaran_melewati_baris_kosong() {
        let dir = project(&["keluaran.log"]);
        let file = dir.join("keluaran.log");
        fs::write(&file, "satu\n\ndua  \n   \ntiga\n\n").unwrap();
        assert_eq!(tail(&file, 2).unwrap(), ["dua", "tiga"]);
        assert_eq!(tail(&file, 9).unwrap(), ["satu", "dua", "tiga"]);
    }
}
