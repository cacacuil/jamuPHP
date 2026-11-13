# JamuPHP

Paket server PHP untuk development dan production: satu perintah mengenali project,
lalu menyalakan dan mengawasi Caddy dan php-fpm. Tanpa file konfigurasi.

Status: spike S1 dan S3. Kode ini sekali buang dan hanya berjalan di Linux. Rencana lengkap,
hasil pengukuran, dan hasil uji ada di [docs/HANDOFF.md](docs/HANDOFF.md).

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

## Container

Image berisi `jamu`, Caddy, dan php-fpm, untuk dipakai di belakang proxy yang mengurus
HTTPS (misalnya Traefik di Dokploy). Port 8080, tidak ada yang berjalan sebagai root.

```bash
docker build -t jamuphp:8.4 .
```

Untuk project Laravel atau PHP biasa, `Dockerfile` project cukup berisi:

```dockerfile
FROM jamuphp:8.4
COPY --chown=www-data:www-data . /app
```

Uji asap image:

```bash
tests/image.sh jamuphp:8.4
```

## Lisensi

AGPL-3.0. Lihat [LICENSE](LICENSE).
