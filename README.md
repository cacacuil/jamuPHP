# JamuPHP

Paket server PHP untuk development dan production: satu perintah mengenali project,
lalu menyalakan dan mengawasi Caddy dan php-fpm. Tanpa file konfigurasi.

Status: spike S1 sampai S4. Kode ini sekali buang. Berjalan di Linux (php-fpm) dan
Windows (php-cgi dari zip resmi PHP). Rencana lengkap, hasil pengukuran, dan hasil uji ada
di [docs/HANDOFF.md](docs/HANDOFF.md).

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

Bila `caddy` atau `php-fpm` tidak ada di `PATH`, tunjuk lewat `JAMU_CADDY` dan `JAMU_PHP_FPM`.

## Windows

Tidak ada yang perlu dipasang. Satu folder berisi `jamu.exe`, `caddy.exe`, dan folder `php`
(isi zip NTS resmi dari windows.php.net) sudah cukup:

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
