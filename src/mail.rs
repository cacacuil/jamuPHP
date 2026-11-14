//! Mode teruskan email: `mail()` di PHP diserahkan ke penyedia SMTP di luar. jamu tidak
//! menjalankan server email; ia hanya menyambungkan `sendmail_path` PHP ke `msmtp`, yang
//! mengantar satu email ke relay lalu selesai.
//!
//! Menyala bila variabel `JAMU_SMTP_HOST` diisi. Sandi dibaca dari lingkungan saat email
//! dikirim dan tidak pernah ditulis ke berkas.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, io};

use crate::paths::find_bin;

/// Batas email per jam untuk satu situs bila `JAMU_MAIL_LIMIT` tidak diisi. Satu situs yang
/// dibobol tidak boleh menghabiskan nama baik relay.
const DEFAULT_LIMIT: u32 = 200;

#[derive(Debug, PartialEq)]
pub struct Relay {
    pub host: String,
    pub port: u16,
    user: Option<String>,
    tls: bool,
    /// Pengirim amplop bila skrip tidak menyebut pengirim sama sekali.
    from: Option<String>,
    /// Email per jam; 0 berarti tanpa batas.
    limit: u32,
}

/// Alamat email yang aman dijadikan argumen: tanpa spasi, dan tidak bisa terbaca sebagai opsi.
fn address(s: &str) -> bool {
    let ok = |part: &str, extra: &[u8]| {
        !part.is_empty()
            && !part.starts_with('-')
            && part
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || extra.contains(&c))
    };
    s.len() <= 254
        && s.split_once('@')
            .is_some_and(|(local, domain)| ok(local, b"._%+=-") && ok(domain, b".-"))
}

/// Pengaturan relay dari variabel lingkungan. `None` bila mode teruskan tidak diminta.
fn relay_from(get: impl Fn(&str) -> Option<String>) -> Result<Option<Relay>, String> {
    let get = |name: &str| get(name).filter(|v| !v.is_empty());
    let Some(host) = get("JAMU_SMTP_HOST") else {
        return Ok(None);
    };
    if cfg!(windows) {
        return Err(
            "JAMU_SMTP_HOST: mode teruskan email belum ada di Windows. Kosongkan variabel itu."
                .into(),
        );
    }
    if host.starts_with('-') || host.contains(|c: char| c.is_whitespace() || c.is_control()) {
        return Err(
            "JAMU_SMTP_HOST: tulis nama host relay saja, misalnya smtp.penyedia.com".into(),
        );
    }
    let port = match get("JAMU_SMTP_PORT") {
        None => 587,
        Some(p) => p
            .parse()
            .ok()
            .filter(|p| *p != 0)
            .ok_or(format!("JAMU_SMTP_PORT=\"{p}\": harus angka 1-65535"))?,
    };
    let tls = match get("JAMU_SMTP_TLS").as_deref() {
        None | Some("on") => true,
        Some("off") => false,
        Some(v) => return Err(format!("JAMU_SMTP_TLS=\"{v}\": isi dengan on atau off")),
    };
    let user = get("JAMU_SMTP_USER");
    if user.is_some() && get("JAMU_SMTP_PASS").is_none() {
        return Err("JAMU_SMTP_USER diisi tetapi JAMU_SMTP_PASS kosong".into());
    }
    if user.is_some() && !tls {
        return Err(
            "JAMU_SMTP_TLS=off tidak bisa dipakai bersama JAMU_SMTP_USER: sandi akan terkirim tanpa enkripsi"
                .into(),
        );
    }
    let from = get("JAMU_MAIL_FROM");
    if let Some(f) = from.as_deref().filter(|f| !address(f)) {
        return Err(format!(
            "JAMU_MAIL_FROM=\"{f}\": tulis satu alamat email, misalnya situs@domain.com"
        ));
    }
    let limit = match get("JAMU_MAIL_LIMIT") {
        None => DEFAULT_LIMIT,
        Some(n) => n.parse().map_err(|_| {
            format!("JAMU_MAIL_LIMIT=\"{n}\": harus angka (email per jam; 0 berarti tanpa batas)")
        })?,
    };
    Ok(Some(Relay {
        host,
        port,
        user,
        tls,
        from,
        limit,
    }))
}

pub fn relay() -> Result<Option<Relay>, String> {
    relay_from(|name| env::var(name).ok())
}

fn msmtp() -> Result<PathBuf, String> {
    find_bin("JAMU_MSMTP", &["msmtp"])
        .ok()
        .filter(|p| p.is_file())
        .ok_or_else(|| {
            "JAMU_SMTP_HOST diisi, tetapi msmtp tidak ditemukan. Pasang msmtp, atau tunjuk lewat variabel JAMU_MSMTP."
                .to_string()
        })
}

impl Relay {
    /// Opsi msmtp untuk menghubungi relay. Berkas konfigurasi msmtp milik pengguna tidak
    /// dibaca: semua pengaturan datang dari sini.
    fn options(&self) -> Vec<String> {
        let mut a = vec![
            "--file=/dev/null".to_string(),
            format!("--host={}", self.host),
            format!("--port={}", self.port),
            // mail() menahan satu pekerja PHP sampai email diterima relay.
            "--timeout=20".into(),
            format!("--tls={}", if self.tls { "on" } else { "off" }),
        ];
        // Port 465 langsung TLS; port lain mulai polos lalu naik ke TLS (STARTTLS).
        if self.tls && self.port == 465 {
            a.push("--tls-starttls=off".into());
        }
        if let Some(user) = &self.user {
            a.push("--auth=on".into());
            a.push(format!("--user={user}"));
            a.push("--passwordeval=printenv JAMU_SMTP_PASS".into());
        }
        a
    }

    /// Bertanya ke relay apakah ia menjawab (dan, dengan TLS, apakah sertifikatnya sah).
    /// Tidak mengirim email dan tidak menguji sandi.
    pub fn probe(&self) -> Result<(), String> {
        let out = Command::new(msmtp()?)
            .args(self.options())
            .arg("--serverinfo")
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("msmtp tidak bisa dijalankan: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(reason(&out.stderr))
        }
    }
}

/// Keluhan msmtp dalam satu baris.
fn reason(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let lines: Vec<&str> = text
        .lines()
        .map(|l| l.trim().trim_start_matches("msmtp: "))
        .filter(|l| !l.is_empty() && *l != "could not send mail")
        .collect();
    if lines.is_empty() {
        "msmtp berhenti tanpa pesan".into()
    } else {
        lines.join("; ")
    }
}

/// Path yang akan ditulis ke `sendmail_path`. Nilai itu dibaca dua pengurai: php.ini (yang
/// memberi arti pada `=`, `;`, dan tanda kutip) lalu shell. Path berspasi atau bertanda
/// baca ditolak daripada dikutip dengan cara yang rapuh.
fn shell_word(p: &Path) -> Result<&str, String> {
    p.to_str()
        .filter(|s| {
            s.bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"/._-".contains(&c))
        })
        .ok_or(format!(
            "Mode teruskan email tidak bisa dipakai: path {} memuat spasi atau tanda baca.",
            p.display()
        ))
}

/// Menambahkan `sendmail_path` ke pengaturan php.ini project bila mode teruskan menyala.
/// `sendmail_path` yang ditulis sendiri di jamu.toml menang.
pub fn add_sendmail(ini: &mut Vec<(String, String)>, state: &Path) -> Result<(), String> {
    if relay()?.is_none() || ini.iter().any(|(k, _)| k == "sendmail_path") {
        return Ok(());
    }
    msmtp()?;
    let exe = env::current_exe().map_err(|e| format!("letak jamu tidak diketahui: {e}"))?;
    ini.push((
        "sendmail_path".into(),
        format!("{} sendmail {}", shell_word(&exe)?, shell_word(state)?),
    ));
    Ok(())
}

/// Apakah `add_sendmail` akan menambahkan sesuatu. Dipakai PHP baris perintah, yang baru
/// membuka folder state bila memang ada yang perlu ditulis.
pub fn wanted(ini: &[(String, String)]) -> Result<bool, String> {
    Ok(relay()?.is_some() && !ini.iter().any(|(k, _)| k == "sendmail_path"))
}

/// Bagian kepala email: semua baris sebelum baris kosong pertama.
fn head(message: &[u8]) -> String {
    let text = String::from_utf8_lossy(message);
    let mut out = String::new();
    for line in text.lines() {
        if line.is_empty() {
            break;
        }
        out += line;
        out.push('\n');
    }
    out
}

/// Isi satu header (baris pertamanya saja), tanpa karakter kendali, untuk ditulis ke log.
fn header(head: &str, name: &str) -> Option<String> {
    head.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.eq_ignore_ascii_case(name).then(|| {
            v.trim()
                .chars()
                .filter(|c| !c.is_control())
                .take(120)
                .collect()
        })
    })
}

/// Pengirim dari `-f` yang ditambahkan PHP di belakang `sendmail_path`. Hanya itu yang
/// diambil dari argumen PHP: parameter kelima `mail()` bisa datang dari pengunjung situs,
/// dan opsi lain (misalnya yang menulis berkas log) tidak boleh sampai ke msmtp.
fn sender_arg(args: &[String]) -> Option<&str> {
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let Some(rest) = a.strip_prefix("-f") else {
            continue;
        };
        let value = if rest.is_empty() {
            it.next().map(String::as_str).unwrap_or_default()
        } else {
            rest
        };
        if address(value) {
            return Some(value);
        }
    }
    None
}

enum Quota {
    Ok,
    /// Email pertama yang melewati batas: saat untuk memberi kabar.
    Reached,
    Over,
}

/// Menghitung email jam ini di berkas kecil di folder state. Kunci berkas mencegah dua
/// pekerja PHP membaca angka yang sama.
fn count(state: &Path, limit: u32, now: u64) -> io::Result<Quota> {
    if limit == 0 {
        return Ok(Quota::Ok);
    }
    let mut f: File = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(state.join("mail-count"))?;
    f.lock()?;
    let mut text = String::new();
    f.read_to_string(&mut text)?;
    let hour = now / 3600;
    let mut parts = text.split_whitespace().map(str::parse::<u64>);
    let sent = match (parts.next(), parts.next()) {
        (Some(Ok(h)), Some(Ok(n))) if h == hour => n,
        _ => 0,
    };
    f.set_len(0)?;
    f.seek(SeekFrom::Start(0))?;
    write!(f, "{hour} {}", sent + 1)?;
    Ok(match sent {
        n if n < limit as u64 => Quota::Ok,
        n if n == limit as u64 => Quota::Reached,
        _ => Quota::Over,
    })
}

/// `jamu sendmail FOLDER_STATE [argumen dari PHP]`: membaca satu email dari stdin dan
/// meneruskannya ke relay. Kode keluar bukan nol membuat `mail()` mengembalikan false.
pub fn sendmail(args: &[String]) -> Result<(), String> {
    let called_by_hand = "jamu sendmail dipanggil oleh PHP, bukan diketik sendiri.";
    let state = args
        .first()
        .filter(|a| Path::new(a).is_absolute())
        .ok_or(called_by_hand)?;
    let relay = relay()?.ok_or(called_by_hand)?;
    let mut message = Vec::new();
    io::stdin()
        .read_to_end(&mut message)
        .map_err(|e| format!("email tidak terbaca: {e}"))?;
    let head = head(&message);
    let to = header(&head, "to").unwrap_or_else(|| "(tanpa penerima)".into());

    let from = match sender_arg(&args[1..]) {
        Some(addr) => format!("--from={addr}"),
        None if header(&head, "from").is_some() => "--read-envelope-from".into(),
        None => match &relay.from {
            Some(addr) => format!("--from={addr}"),
            None => {
                return Err(format!(
                    "email ke {to} tidak punya pengirim: tambahkan header From, atau setel JAMU_MAIL_FROM"
                ));
            }
        },
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default();
    match count(Path::new(state), relay.limit, now) {
        Ok(Quota::Ok) => {}
        Ok(Quota::Reached) => {
            return Err(format!(
                "email ke {to} ditolak: batas {} email per jam tercapai (JAMU_MAIL_LIMIT). \
                 Email berikutnya ditolak tanpa kabar sampai jam berganti.",
                relay.limit
            ));
        }
        Ok(Quota::Over) => std::process::exit(1),
        Err(e) => {
            return Err(format!(
                "email ke {to} ditolak: penghitung batas tidak bisa ditulis ({e})"
            ));
        }
    }

    let mut child = Command::new(msmtp()?)
        .args(relay.options())
        .arg(from)
        // Penerima dibaca dari header To, Cc, dan Bcc; Bcc dibuang dari email yang dikirim.
        .arg("--read-recipients")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("msmtp tidak bisa dijalankan: {e}"))?;
    // Bila msmtp sudah menyerah sebelum membaca, tulisannya gagal; kode keluarnya yang dipakai.
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(&message);
    }
    let out = child
        .wait_with_output()
        .map_err(|e| format!("msmtp tidak bisa ditunggu: {e}"))?;
    if !out.status.success() {
        return Err(format!("email ke {to} gagal: {}", reason(&out.stderr)));
    }
    say!("email ke {to} diteruskan ke {}", relay.host);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            pairs
                .iter()
                .find(|(k, _)| *k == name)
                .map(|(_, v)| v.to_string())
        }
    }

    #[test]
    #[cfg(unix)]
    fn relay_dibaca_dari_lingkungan() {
        assert_eq!(relay_from(env(&[])).unwrap(), None);
        assert_eq!(relay_from(env(&[("JAMU_SMTP_HOST", "")])).unwrap(), None);
        let r = relay_from(env(&[("JAMU_SMTP_HOST", "smtp.contoh.test")]))
            .unwrap()
            .unwrap();
        assert_eq!((r.port, r.tls, r.limit), (587, true, DEFAULT_LIMIT));
        assert_eq!(
            r.options(),
            [
                "--file=/dev/null",
                "--host=smtp.contoh.test",
                "--port=587",
                "--timeout=20",
                "--tls=on"
            ]
        );

        let r = relay_from(env(&[
            ("JAMU_SMTP_HOST", "smtp.contoh.test"),
            ("JAMU_SMTP_PORT", "465"),
            ("JAMU_SMTP_USER", "siti@contoh.test"),
            ("JAMU_SMTP_PASS", "rahasia besar"),
        ]))
        .unwrap()
        .unwrap();
        let o = r.options();
        assert!(o.contains(&"--tls-starttls=off".to_string()));
        assert!(o.contains(&"--auth=on".to_string()));
        assert!(o.contains(&"--user=siti@contoh.test".to_string()));
        // Sandi tidak pernah menjadi argumen: ia dibaca msmtp dari lingkungan.
        assert!(o.contains(&"--passwordeval=printenv JAMU_SMTP_PASS".to_string()));
        assert!(!o.iter().any(|a| a.contains("rahasia")));
    }

    #[test]
    #[cfg(unix)]
    fn pengaturan_relay_yang_salah_ditolak() {
        let host = ("JAMU_SMTP_HOST", "smtp.contoh.test");
        for (bad, word) in [
            (("JAMU_SMTP_PORT", "abc"), "JAMU_SMTP_PORT"),
            (("JAMU_SMTP_PORT", "0"), "JAMU_SMTP_PORT"),
            (("JAMU_SMTP_TLS", "mungkin"), "JAMU_SMTP_TLS"),
            (("JAMU_SMTP_USER", "siti"), "JAMU_SMTP_PASS"),
            (("JAMU_MAIL_FROM", "bukan alamat"), "JAMU_MAIL_FROM"),
            (("JAMU_MAIL_LIMIT", "-1"), "JAMU_MAIL_LIMIT"),
        ] {
            let err = relay_from(env(&[host, bad])).unwrap_err();
            assert!(err.contains(word), "{err}");
        }
        assert!(relay_from(env(&[("JAMU_SMTP_HOST", "--file=/etc/passwd")])).is_err());
        assert!(relay_from(env(&[("JAMU_SMTP_HOST", "a b")])).is_err());
        let err = relay_from(env(&[
            host,
            ("JAMU_SMTP_TLS", "off"),
            ("JAMU_SMTP_USER", "siti"),
            ("JAMU_SMTP_PASS", "x"),
        ]))
        .unwrap_err();
        assert!(err.contains("tanpa enkripsi"));
    }

    #[test]
    fn hanya_pengirim_yang_diambil_dari_argumen_php() {
        let args = |s: &str| s.split(' ').map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(
            sender_arg(&args("-t -i -fa@situs.test")),
            Some("a@situs.test")
        );
        assert_eq!(
            sender_arg(&args("-f a@situs.test -X/tmp/x")),
            Some("a@situs.test")
        );
        assert_eq!(sender_arg(&args("-t -i")), None);
        for bad in [
            "-f-oQ/tmp",
            "-f",
            "-fa@b.c;id",
            "-fbukan-alamat",
            "-f a@b.c\"",
            "-X/tmp/log",
        ] {
            assert_eq!(sender_arg(&args(bad)), None, "{bad}");
        }
    }

    #[test]
    fn header_dibaca_dari_kepala_saja() {
        let h = head(b"To: a@contoh.test\r\nSUBJECT: hai\r\n\r\nFrom: palsu@isi.test\r\n");
        assert_eq!(header(&h, "to").as_deref(), Some("a@contoh.test"));
        assert_eq!(header(&h, "subject").as_deref(), Some("hai"));
        assert_eq!(header(&h, "from"), None);
        let h = head(b"To: a@contoh.test\x1b[31m\n\nisi");
        assert_eq!(header(&h, "to").as_deref(), Some("a@contoh.test[31m"));
    }

    #[test]
    fn batas_per_jam_dihitung_dan_diulang_saat_jam_berganti() {
        let dir = crate::testutil::project(&["x"]);
        let kinds = |now: u64| match count(&dir, 2, now).unwrap() {
            Quota::Ok => "ok",
            Quota::Reached => "tercapai",
            Quota::Over => "lewat",
        };
        assert_eq!(
            [kinds(7200), kinds(7300), kinds(7400), kinds(7500)],
            ["ok", "ok", "tercapai", "lewat"]
        );
        assert_eq!(kinds(10800), "ok");
        // Berkas yang rusak dianggap kosong, bukan alasan menolak email.
        std::fs::write(dir.join("mail-count"), "sampah").unwrap();
        assert_eq!(kinds(10900), "ok");
        assert!(matches!(count(&dir, 0, 10900).unwrap(), Quota::Ok));
    }

    #[test]
    fn path_untuk_sendmail_path_harus_satu_kata() {
        assert!(shell_word(Path::new("/var/lib/jamu/app-1a2b3c4d")).is_ok());
        assert!(shell_word(Path::new("/home/siti/folder saya/jamu")).is_err());
        assert!(shell_word(Path::new("/tmp/x;id")).is_err());
    }
}
