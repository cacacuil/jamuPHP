# JamuPHP

Paket server PHP untuk development dan production: satu perintah mengenali project,
lalu menyalakan dan mengawasi Caddy dan php-fpm. Tanpa file konfigurasi.

Status: fase 2 (production) sedang dikerjakan, belum untuk dipakai pelanggan. Berjalan
di Linux (php-fpm) dan Windows (php-cgi dari zip resmi PHP). Rencana lengkap, hasil
pengukuran, dan hasil uji ada di [docs/HANDOFF.md](docs/HANDOFF.md).

## Mencoba

Butuh Rust, `caddy`, dan `php-fpm` (8.2 ke atas) di mesin.

```bash
cargo build --release
```

Development, di folder project Laravel, WordPress, atau PHP biasa:

```bash
./target/release/jamu serve /path/ke/project
```

Production, dengan sertifikat yang diterbitkan dan diperbarui otomatis:

```bash
./target/release/jamu serve /path/ke/project --domain contoh.com --email anda@contoh.com
```

`--domain` boleh diulang, misalnya untuk nama dengan dan tanpa `www`. Terbit, gagal, dan
diperpanjangnya sertifikat dicatat di log sebagai baris `jamu: sertifikat ...`; bila gagal,
alasan dari CA ikut ditampilkan dan penerbitan dicoba lagi sendiri.

Bila `caddy` atau `php-fpm` tidak ada di `PATH`, tunjuk lewat `JAMU_CADDY` dan `JAMU_PHP_FPM`.

Memeriksa project sebelum dinyalakan atau dikirim ke server, dengan opsi yang sama seperti
`serve`. Keluar dengan kode 1 bila ada masalah, jadi bisa dipasang di CI:

```bash
jamu check /path/ke/project --behind-proxy
```

Yang diperiksa: `jamu.toml`, versi PHP, konfigurasi Caddy dan php-fpm yang akan dipakai,
ekstensi yang diminta paket Composer, folder yang harus bisa ditulis, `APP_KEY`,
`APP_DEBUG`, dan cache konfigurasi Laravel, rekaman DNS untuk `--domain`, dan folder yang ditutup lewat
`.htaccess` (`jamu` tidak membaca `.htaccess`, jadi folder seperti itu terbuka), dan relay
SMTP bila mode teruskan email dipakai.

PHP dan Composer untuk project ini, yaitu PHP yang sama dengan yang melayani situsnya:

```bash
jamu php artisan migrate
```

```bash
jamu composer install
```

Laravel di server: buat cache konfigurasinya tiap kali kode atau variabel lingkungan
berubah. Tanpa cache itu Laravel membaca ulang `.env` dan semua berkas config di tiap
permintaan; pada aplikasi baru selisihnya terukur 584 lawan 978 permintaan per detik.
`jamu check` dan `jamu serve` menyebutnya bila cache itu belum ada.

```bash
jamu php artisan optimize
```

## jamu.toml

Tidak wajib. Tanpa file ini `jamu` mengenali sendiri jenis project dan document root-nya.
Taruh `jamu.toml` di folder project hanya bila ada yang perlu ditimpa:

```toml
php  = "8.4"            # tolak jalan dengan versi PHP lain
root = "public"         # document root
scheduler = true        # penjadwal; bawaan: menyala kecuali di mode dev
timeout = 300           # batas waktu satu permintaan PHP, detik; 0 = tanpa batas

[ini]
memory_limit = "512M"

[[worker]]
name = "queue"
run  = "php artisan queue:work --tries=3"
```

Pekerja dinyalakan bersama situs, diawasi, dan dinyalakan ulang bila mati. Penjadwal
menggantikan baris cron: tiap menit ia menjalankan `schedule:run` untuk Laravel dan
`wp-cron.php` untuk WordPress. Kunci yang tidak dikenal ditolak.

Batas yang dijaga `jamu` supaya satu situs tidak menjatuhkan dirinya sendiri:

- **Waktu.** Permintaan PHP yang belum selesai setelah `timeout` detik (bawaan 300) dijawab
  504 dan pekerjanya dihentikan, supaya permintaan yang menggantung tidak menghabiskan
  semua pekerja. Jawaban yang dialirkan (SSE, unduhan buatan PHP) ikut terpotong di batas
  yang sama; naikkan `timeout` atau isi 0 bila situs membutuhkannya. Di Windows jawabannya
  504 tetapi pekerjanya tidak dihentikan.
- **Ukuran unggahan.** Mengikuti `post_max_size` (bawaan 128M). Mengubahnya di `[ini]`
  sudah cukup; yang lebih besar ditolak 413 sebelum sampai ke PHP.
- **Jumlah pekerja PHP.** Dua kali jumlah core, tetapi tidak lebih dari yang muat di batas
  memori container (64 MB per pekerja), dan tidak kurang dari 4. Angkanya tertulis di
  baris pertama log saat `jamu` menyala.

## WordPress

Tidak ada yang perlu diatur. Yang sudah diurus `jamu`:

- Berkas PHP di folder unggahan tidak dijalankan.
- Multisite subdirektori dikenali dari `wp-config.php` dan aturan tulis ulangnya dipasang
  sendiri. Nyalakan ulang `jamu` setelah mengubah situs menjadi multisite.
- Bila WP Super Cache terpasang dan aktif, halaman yang sudah di-cache disajikan langsung
  dari berkas buatan plugin tanpa menyentuh PHP. Pengunjung yang login, yang berkomentar,
  dan permintaan dengan query string tetap dilayani PHP. Mode "mobile device support"
  plugin itu tidak didukung; biarkan mati (bawaannya memang mati).
- `wp-cron.php` dijalankan tiap menit, jadi tulisan terjadwal terbit tepat waktu walau
  situs sedang sepi.

## Email

`jamu` tidak menjalankan server email. Bila variabel `JAMU_SMTP_HOST` diisi, `mail()` di
PHP diteruskan ke penyedia SMTP itu (lewat `msmtp`, yang sudah ada di image container):

```bash
JAMU_SMTP_HOST=smtp.penyedia.com JAMU_SMTP_USER=akun JAMU_SMTP_PASS=sandi jamu serve /path/ke/project --behind-proxy
```

| Variabel | Bawaan | Arti |
|---|---|---|
| `JAMU_SMTP_HOST` | kosong | Nama host relay. Kosong berarti `mail()` tidak disentuh. |
| `JAMU_SMTP_PORT` | `587` | Port 465 langsung TLS; port lain memakai STARTTLS. |
| `JAMU_SMTP_USER`, `JAMU_SMTP_PASS` | kosong | Akun di relay. Sandi tidak ditulis ke berkas. |
| `JAMU_SMTP_TLS` | `on` | `off` hanya untuk relay di jaringan sendiri atau penangkap email saat development; tidak bisa dipakai bersama akun. |
| `JAMU_MAIL_FROM` | kosong | Pengirim untuk email yang skripnya tidak menyebut pengirim. |
| `JAMU_MAIL_LIMIT` | `200` | Email per jam untuk situs ini; `0` berarti tanpa batas. |

Tiap email dicatat satu baris di log, terkirim maupun gagal beserta alasan dari relay.
Email yang melewati batas ditolak (`mail()` mengembalikan `false`), supaya satu situs yang
dibobol tidak merusak nama baik relay. `jamu check` ikut menanyai relay. Laravel tidak
butuh ini: ia berbicara SMTP sendiri lewat `MAIL_*` di `.env`. Belum ada di Windows.

## Log

Semua keluar di stderr, satu baris per kejadian, dengan awalan sumbernya: `jamu:`,
`[caddy]`, `[php-fpm]`, `[schedule]`, nama pekerja, dan `[akses]` untuk tiap permintaan:

```
[akses] 203.0.113.9 GET /produk/ 200 12 ms
```

## Windows

Tidak ada yang perlu dipasang. Satu folder berisi `jamu.exe`, `caddy.exe`, `composer.phar`,
dan folder `php` (isi zip NTS resmi dari windows.php.net) sudah cukup:

```
jamu.exe serve C:\path\ke\project
```

Rakitan seperti itu dibuat CI di setiap push dan bisa diunduh dari tab Actions, artifact
`jamuphp-windows-php8.4`. Uji yang sama dengan Linux dijalankan di sana, ditambah uji
khusus Windows:

```bash
bash tests/e2e.sh
```

```bash
bash tests/windows.sh
```

## Container

Image berisi `jamu`, Caddy, dan php-fpm, untuk dipakai di belakang proxy yang mengurus
HTTPS (misalnya Traefik di Dokploy). Port 8080, tidak ada yang berjalan sebagai root.

Untuk project Laravel atau PHP biasa, `Dockerfile` project cukup berisi:

```dockerfile
FROM ghcr.io/cacacuil/jamuphp:8.4
COPY --chown=www-data:www-data . /app
RUN jamu composer install --no-dev --optimize-autoloader
```

Image diterbitkan CI ke GHCR dari branch `main`, setelah lulus uji asap. Di Dokploy,
arahkan domain ke port 8080.

Membangun sendiri:

```bash
docker build -t jamuphp:8.4 .
```

Uji asap image:

```bash
tests/image.sh jamuphp:8.4
```

## Pengukuran

`docs/benchmark/s4.sh` mengukur WordPress di image ini dengan MariaDB dan WP Super Cache,
tiap beban tiga kali jalan; lingkungannya disiapkan `docs/benchmark/s4-setup.sh`. Hasil
mentah ada di `docs/benchmark/s4-results.txt` dan diringkas `docs/benchmark/s4-ringkas.sh`.
Pembahasannya di bagian 19 [docs/HANDOFF.md](docs/HANDOFF.md).

## Lisensi

AGPL-3.0. Lihat [LICENSE](LICENSE).
