## Banding 2026-11-14, worker mode: Laravel di jamu, FrankenPHP worker, Octane RoadRunner, Octane Swoole

4 core, 16 GB, wrk 8 detik x 3 (median). Run CI 43629823038, branch `banding-worker` (`MODE=worker`).
Octane: 8 pekerja, opcache aktif di PHP baris perintah, pekerja diganti tiap 1000 permintaan,
di belakang Caddy yang sama dengan jamu. Tabel terakhir: satu pekerja PHP dimatikan dengan
SIGSEGV (`/mati`) selagi `/up` dibebani 16 koneksi.

```
aplikasi: Laravel Framework 13.42.0
php: octane-roadrunner = PHP 8.4.27 cli opcache=on
php: octane-swoole = PHP 8.4.27 cli opcache=on
server: jamu = v2.11.9 (Caddy) + php-fpm ondemand
server: franken = FrankenPHP v1.13.5 PHP 8.4.27 Caddy v2.11.9 h1:bhZtcRKQOq0fuLvEgCdPYvUxXyGn9kzjG4lh2332DTE=
server: nginx = nginx version: nginx/1.26.3 + php-fpm static
server: octane-roadrunner = rr version 2025.1.15 (build time: 2026-06-17T16:05:14+0000, go1.26.4), OS: linux, arch: amd64 di belakang Caddy
server: octane-swoole = Swoole 6.2.5 di belakang Caddy
image: laravel jamu=764 franken=633 nginx=772 MB
image: laravel octane-roadrunner=824 octane-swoole=836 MB
```

Koneksi `64z`: 64 koneksi yang meminta jawaban dimampatkan, seperti browser.

| beban | koneksi | server | req/s | p50 ms | p99 ms | galat | memori MB |
|---|---|---|---|---|---|---|---|
| laravel / | 8 | jamu | 1046.0 | 7.28 | 15.65 | 0 | - |
| laravel / | 8 | franken-worker | 1958.0 | 3.86 | 8.52 | 0 | - |
| laravel / | 8 | octane-roadrunner | 961.5 | 7.79 | 248.42 | 0 | - |
| laravel / | 8 | octane-swoole | 1083.4 | 6.91 | 86.63 | 0 | - |
| laravel / | 64 | jamu | 1013.1 | 61.56 | 94.31 | 0 | 79 |
| laravel / | 64z | jamu | 901.9 | 69.77 | 88.62 | 0 | - |
| laravel / | 64 | franken-worker | 2049.7 | 30.89 | 44.09 | 0 | 77 |
| laravel / | 64z | franken-worker | 1398.1 | 45.30 | 70.22 | 0 | - |
| laravel / | 64 | octane-roadrunner | 939.1 | 64.85 | 103.49 | 0 | 437 |
| laravel / | 64z | octane-roadrunner | 767.2 | 81.35 | 138.70 | 0 | - |
| laravel / | 64 | octane-swoole | 1076.5 | 56.67 | 355.74 | 0 | 347 |
| laravel / | 64z | octane-swoole | 1239.2 | 48.05 | 359.48 | 0 | - |
| laravel /up | 8 | jamu | 1429.8 | 5.36 | 11.98 | 0 | - |
| laravel /up | 8 | franken-worker | 3082.9 | 2.46 | 5.42 | 0 | - |
| laravel /up | 8 | octane-roadrunner | 1574.4 | 4.56 | 370.26 | 0 | - |
| laravel /up | 8 | octane-swoole | 2167.9 | 3.34 | 175.24 | 0 | - |
| laravel /up | 64 | jamu | 1410.8 | 44.50 | 58.29 | 0 | 74 |
| laravel /up | 64z | jamu | 1380.6 | 45.57 | 57.47 | 0 | - |
| laravel /up | 64 | franken-worker | 3464.9 | 18.25 | 31.53 | 0 | 73 |
| laravel /up | 64z | franken-worker | 3180.1 | 19.85 | 36.69 | 0 | - |
| laravel /up | 64 | octane-roadrunner | 1548.1 | 36.96 | 118.59 | 0 | 427 |
| laravel /up | 64z | octane-roadrunner | 1556.4 | 38.41 | 73.89 | 0 | - |
| laravel /up | 64 | octane-swoole | 2273.5 | 25.96 | 390.13 | 0 | 332 |
| laravel /up | 64z | octane-swoole | 2391.5 | 24.89 | 386.72 | 0 | - |

| aplikasi | server | siap (ms) | memori diam MB |
|---|---|---|---|
| laravel | jamu | 350 | 45 |
| laravel | franken-worker | 460 | 106 |
| laravel | octane-roadrunner | 780 | 304 |
| laravel | octane-swoole | 699 | 299 |

| server | jawaban permintaan yang crash | req/s | galat | permintaan | putus (ms) | denyut gagal | container diulang |
|---|---|---|---|---|---|---|---|
| jamu/laravel | 502 | 1258.2 | 0 | 15118 | 0 | 0 | 0 |
| franken-worker/laravel | 000 | 2840.0 | 64310 | 34299 | 807 | 8 | 1 |
| octane-roadrunner/laravel | 500 | 1439.0 | 0 | 17292 | 0 | 0 | 0 |
| octane-swoole/laravel | 000 | 2084.5 | 0 | 25041 | 0 | 0 | 0 |
