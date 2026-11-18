# JamuPHP: handoff v2

Tanggal: 18 November 2026, revisi 17. Menggantikan handoff v1 ("XAMPP berbasis Rust").
Nama produk: JamuPHP. Perintahnya `jamu`, file config-nya `jamu.toml`. Repositori:
https://github.com/cacacuil/jamuphp (lisensi AGPL-3.0).

Perubahan di revisi 2: `.htaccess` didukung (bagian 7), project dikenali tanpa config
(bagian 6), dan jalur negatif dipetakan (bagian 9).
Perubahan di revisi 3: Redis, HTTP/3, multisite, dan worker mode diberi tempat (bagian 5,
"Rakit, jangan bangun"); multisite dan Redis keluar dari daftar "tidak dikerjakan".
Perubahan di revisi 4: lapisan web diukur (bagian 5, "Pengukuran lapisan web"). Owner
menunda soal `.htaccess` dan menanyakan Caddy + php-fpm. Hasil ukur mengubah rekomendasi
lapisan web menjadi Apache (MPM event) + php-fpm ondemand.
Perubahan di revisi 5: owner mensyaratkan SSL sah bawaan karena targetnya production.
Rekomendasi dilengkapi mod_md dan diuji ujung ke ujung (bagian 5, "SSL sah bawaan");
jalur negatif SSL ditambahkan (bagian 9, G).
Perubahan di revisi 6: konkurensi diukur (bagian 5, "Konkurensi"). Apache tidak bertahan
sebagai server terdepan pada ribuan koneksi. Rekomendasi lapisan web berubah menjadi Caddy
+ php-fpm ondemand; Apache hanya dipakai di belakang Caddy bila `.htaccess` diperlukan.
Perubahan di revisi 7: owner memutuskan Caddy di semua tempat dan memberi nama JamuPHP.
Spike S1 dikerjakan dan lulus (bagian 16). Posisi soal async dan extension Rust dicatat
(bagian 5, "Rakit, jangan bangun").
Perubahan di revisi 8: spike S3 dikerjakan (bagian 17): image container, mode di belakang
proxy, dan cara menangani extension PHP.
Perubahan di revisi 9: spike S2 dikerjakan dan lulus (bagian 18): `jamu` yang sama berjalan
di Windows dengan Caddy dan `php-cgi` dari zip resmi. Rencana "pool php-cgi buatan sendiri"
dibatalkan karena `php-cgi` ternyata sudah mengurus pekerjanya sendiri di Windows.
Perubahan di revisi 10: spike S4 dikerjakan (bagian 19): pengukuran diulang tiga kali jalan
dengan MariaDB dan WP Super Cache di image `jamuphp`. Cache halaman tingkat platform
(jalur negatif H7) diganti rekomendasi yang lebih kecil: aturan Caddy yang menyajikan
berkas cache buatan plugin.
Perubahan di revisi 11: owner menyetujui aturan berkas cache masuk fase 2. Fase 1 dimulai
(bagian 20): `jamu.toml`, pekerja, penjadwal Laravel, `jamu php` dan `jamu composer`, serta
tes daftar wajib bagian 8 yang berjalan sama di Linux dan Windows. Lingkup fase 1 dipotong
ulang mengikuti keputusan Caddy (bagian 10). Contoh `jamu.toml` di bagian 6 dikoreksi.
Perubahan di revisi 12: fase 1 digabung ke `main` atas permintaan owner. Fase 2 dikerjakan
di branch `fase-2` (bagian 21): aturan berkas cache, multisite, wp-cron, log akses,
`jamu check`, beberapa domain dan kabar sertifikat, serta mode teruskan email. Sebab runner
Windows di CI sesekali hilang ditemukan dan dicatat di bagian yang sama.
Perubahan di revisi 13: owner menetapkan sasaran berikutnya adalah performa dan kestabilan;
pengerasan keamanan ditunda. Dikerjakan di branch `stabil-performa` (bagian 22): batas waktu
permintaan, jumlah pekerja mengikuti memori container, batas unggahan satu sumber, dan
kabar php-fpm yang terbaca. Penyetelan opcache dan JIT diukur dan ditolak.
Perubahan di revisi 14: image jamuphp dibandingkan dengan FrankenPHP (mode klasik dan
worker) dan nginx + php-fpm di CI (bagian 23). Dua pemborosan milik jamu sendiri ketemu
dan diperbaiki: aturan jenis berkas dan penguraian log akses.
Perubahan di revisi 15: jalur PHP diprofil (bagian 24). Keluaran PHP kini diteruskan tiap
20 ms dan pencatat FastCGI per permintaan dibuang; Laravel tanpa cache konfigurasi disebut
oleh `check` dan `serve`. Worker mode diukur dengan tiga mesin (FrankenPHP, RoadRunner,
Swoole), termasuk apa yang terjadi saat satu pekerja crash.
Perubahan di revisi 16: `stabil-performa` (bagian 22 sampai 24) digabung ke `main` atas
permintaan owner.
Perubahan di revisi 17: `artisan optimize` otomatis dengan sakelar `optimize` di
`jamu.toml` (bagian 25), di branch `stabil-performa`.

## 1. Inti dalam satu menit

- Satu binary Rust headless yang mengenali project secara otomatis, lalu menjalankan web
  server, pool PHP (lewat FastCGI), worker, dan penjadwal. File config bersifat opsional.
- Binary yang sama dipakai di dua tempat: sebagai proses utama di container production
  (WordPress PaaS di Dokploy) dan sebagai mesin di balik aplikasi desktop.
- `.htaccess` dipatuhi lewat lapisan kompatibilitas Apache, dengan cakupan yang
  didefinisikan dan diuji terhadap Apache asli.
- Urutan kerja: spike, core headless, container production, aplikasi desktop Windows,
  sinkronisasi ke hosting, macOS.
- Yang dicoret dari v1: embed SAPI, worker mode buatan sendiri, Pingora, FTP di dalam core.
- Pembeda produk: aturan yang sama di laptop dan di hosting (termasuk `.htaccess`), tanpa
  config, ditambah perintah tarik/dorong situs ke hosting.

**Status lapisan web (diputuskan owner, 13 Nov 2026): Caddy di semua tempat.** Susunannya adalah
Caddy + php-fpm ondemand untuk server yang memegang port 443 sendiri, dengan Rust sebagai
supervisor, deteksi project, dashboard, mail, dan CLI. Apache tidak lagi direkomendasikan
sebagai server terdepan; ia menjadi komponen pilihan di belakang Caddy untuk situs yang
membutuhkan `.htaccess`. Riwayatnya: revisi 1-3 merekomendasikan server Rust sendiri,
revisi 4-5 Apache, revisi 6 Caddy. Setiap perubahan mengikuti syarat baru dari owner dan
hasil ukur; dasarnya ada di bagian 5. Bagian 7 dan 8 hanya berlaku bila owner memilih
server Rust sendiri, dan kalimat "web server: hyper" di dokumen ini dibaca sebagai "Caddy
yang dikelola supervisor".

## 2. Keputusan

### Sudah diambil (sesi 13 Nov 2026)

| Keputusan | Isi |
|---|---|
| Target | Keduanya, dengan satu core: dev lokal dan runtime production. |
| `.htaccess` | Didukung. Penting bagi WordPress dan pengguna Apache. |
| Pengalaman | Semulus mungkin untuk pemula, tanpa config rumit. |
| Hasil sesi ini | Awalnya rencana saja; lalu owner meminta spike S1 dikerjakan. |
| Lapisan web | Caddy + php-fpm ondemand di semua tempat: desktop, server mandiri, dan container. |
| Nama | JamuPHP. |
| Lisensi | AGPL-3.0 (dipilih owner saat membuat repositori). |
| Registry image | GHCR: `ghcr.io/cacacuil/jamuphp`. |
| Cache halaman | Tidak ada cache buatan platform. Di fase 2 `jamu` menulis aturan Caddy yang menyajikan berkas cache buatan plugin, hanya bila berkas plugin itu ada (bagian 19). |
| WordPress | Tidak ikut di image dasar. Image dasar hanya PHP, Caddy, dan `jamu`; WordPress sejauh ini hanya dipakai untuk pengujian. |

### Rekomendasi yang menunggu persetujuan owner

| Hal | Rekomendasi | Alasan singkat |
|---|---|---|
| Urutan OS | Linux, lalu Windows, lalu macOS. Windows wajib ikut di CI sejak minggu pertama. | Bagian 11. |
| Lapisan web | Caddy + php-fpm mode ondemand untuk server yang memegang port 443. Untuk container per situs di belakang Traefik: nginx + php-fpm ondemand (hemat sekitar 40 MB per situs) atau Caddy demi keseragaman. | Bagian 5, "Konkurensi". |
| `.htaccess` | Ditunda oleh owner. Bila nanti diperlukan: Apache di belakang Caddy untuk situs yang memintanya. Di posisi itu Apache hanya menerima sedikit koneksi dari Caddy. | Bagian 5 dan 7. |
| SSL sah | Bawaan Caddy: sertifikat terbit dan diperbarui otomatis tanpa reload, HTTP/3 ikut. | Bagian 5, "SSL sah bawaan". |
| Sumber PHP | Build resmi, bukan build sendiri, selama memungkinkan. | Bagian 4, temuan 5. |

### Masih terbuka (harus dijawab owner)

1. Nama produk: sudah diputuskan, JamuPHP.
2. Open source atau tertutup: sudah diputuskan, terbuka dengan AGPL-3.0.
3. Kapasitas tim: berapa orang dan penuh waktu atau sambilan. Dukungan `.htaccess`
   menambah satu komponen besar; fase desktop tidak realistis sebagai kerja sambilan.
4. Badan hukum untuk code signing Windows (risiko 3).

## 3. Hasil verifikasi riset (13 Nov 2026)

Tanggal rilis crate diambil langsung dari API crates.io. Kode sumber WordPress, php-src,
dan plugin dibaca langsung dari GitHub. Sisanya dari pencarian web. wordpress.org dan
static-php.dev tidak bisa diakses dari sesi ini.

| Komponen | Status | Dampak ke rencana |
|---|---|---|
| FrankenPHP | Windows native resmi sejak v1.12.0 (Maret 2026), menaut ke binary PHP resmi, worker mode jalan. Repo di bawah organisasi `php`. | Embed SAPI di Windows sudah dikerjakan pihak lain yang didanai. Fase 3 versi v1 kehilangan alasan. |
| static-php-cli | Aktif. v3 sedang dikembangkan dan berganti nama menjadi StaticPHP. Di Windows hanya membangun CLI dan micro; fpm hanya Linux dan macOS. | Rencana v1 "PHP dari static-php-cli" tidak bisa memberi SAPI FastCGI di Windows. Temuan 5. |
| MailCrab | 1.10.0, 17 Sep 2026. Aktif. Email disimpan di memori. | Layak sebagai referensi. |
| libunftp | 0.24.0, 28 Okt 2026. Aktif. | Sehat, tetapi FTP dikeluarkan dari core. Temuan 6. |
| fastcgi-client | 0.12.0, 29 Jul 2026. Aktif, punya contoh axum ke php-fpm. Ada fork `fcgi-client` yang memperbaiki stack overflow di Windows. | Dipakai di spike. Adanya fork itu tanda harus diuji di Windows lebih awal. |
| Pasir | crates.io: 0.5.4, terbit terakhir 21 Okt 2025. Bukan "v0.1" seperti di v1. | Lebih dari setahun tanpa rilis di crates.io. Referensi kode saja. |
| OxPHP | Repo dibuat Feb 2026, AGPL-3.0. | Referensi saja; lisensinya tidak cocok untuk disalin. |
| Rapira | Angka 111k RPS hanya dari satu tulisan pihak ketiga; repo utama tidak ditemukan. | Abaikan. |
| hyper, rcgen, lettre, hickory-server, russh, tauri | Semua rilis antara Agu dan Nov 2026. | Aman dijadikan dependency. |
| Pingora | 0.9.1, 3 Nov 2026. Aktif. | Tidak dipakai. Temuan 2. |

Kondisi pasar:

- Laragon 7 tidak lagi gratis untuk pemakaian komersial: USD 49 per tahun atau USD 149 seumur hidup.
- Herd gratis untuk PHP, nginx, dan DNS. Database dan mail ada di Herd Pro, USD 99 (sumber
  berbeda soal tahunan atau sekali bayar; situs resminya mengarah ke tahunan).
- EnvKit: pendatang baru, gratis dengan semua fitur, Windows dan macOS, masih beta, tertutup.
- ServBay: macOS dan Windows, ada versi gratis, memakai Caddy.

Celah "gratis, sudah termasuk database dan inbox email, di Windows" memang ada, tetapi
sudah mulai diisi orang lain. Celah itu saja tidak cukup sebagai pembeda.

## 4. Review blak-blakan terhadap v1

Diurutkan dari yang paling merusak rencana.

**1. Fase 3 (embed SAPI) bertentangan dengan fase 2 (switch versi PHP per project).**
Embed berarti satu versi PHP tertaut ke dalam binary. Banyak versi berarti banyak binary,
atau memuat beberapa libphp sekaligus dalam satu proses, yang akan bentrok simbol. Embed
juga mewajibkan build ZTS, dan satu segfault di extension mana pun menjatuhkan web server,
supervisor, dan mail sekaligus. FastCGI memberi multi-versi tanpa usaha, isolasi proses,
dan bisa memakai build PHP resmi. Keputusan: embed SAPI dihapus dari roadmap. Kalau suatu
saat benchmark menunjukkan butuh worker mode, supervisor cukup menjalankan
`php artisan octane:start` sebagai proses dan web server meneruskan request ke sana.

**2. Pingora adalah alat yang salah.** Pingora adalah kerangka reverse proxy skala CDN dan
tidak membawa klien FastCGI. Pakai hyper langsung. axum boleh dipakai untuk API dashboard.

**3. "Config dev identik dengan production" tidak bisa ditepati secara harfiah.**
Windows dengan php-cgi tidak sama dengan Linux dengan php-fpm. Filesystem Windows tidak
membedakan huruf besar-kecil. Yang bisa dibuat sama adalah aturannya: deteksi project,
routing, dan tafsir `.htaccess` dijalankan oleh kode yang sama persis. Janjikan "aturan
yang sama", jangan "lingkungan yang sama".

**4. "Single binary kecil" tidak benar begitu PHP dan MariaDB ikut.** Satu versi PHP
berukuran puluhan MB, MariaDB ratusan MB setelah diekstrak. Yang jujur: pengelola yang
kecil, komponen diunduh saat dibutuhkan dari manifest dengan checksum yang dikunci.

**5. "Rebuild PHP setiap security release" adalah beban yang dibuat sendiri.**
- Windows: zip NTS resmi dari windows.php.net. Sudah termasuk `php-cgi.exe` dan extension.
- Container production: image Docker resmi `php:*-fpm` sebagai dasar, lalu salin binary
  kita ke dalamnya. Tidak ada build PHP sama sekali.
- macOS dan desktop Linux: baru di sini build sendiri (StaticPHP) diperlukan. Itu sebabnya
  macOS dikerjakan terakhir.

Pekerjaan per rilis PHP menjadi: perbarui manifest, jalankan smoke test otomatis. Catatan:
dukungan Xdebug di build statis belum saya verifikasi, dan Xdebug wajib untuk alat dev.

**6. FTP adalah kebiasaan XAMPP yang ikut tersalin.** Di dev lokal file sudah ada di disk
sendiri. Di hosting, pelanggan butuh SFTP, dan port pasif FTP per container di belakang
Traefik merepotkan. Keputusan: keluar dari core. Kalau nanti dibuat, bentuknya gateway
SFTP (russh) yang terpisah di sisi hosting.

**7. Bagian mail mencampur tiga hal.**
- Menangkap email di dev: listener SMTP lokal dan inbox web.
- Fungsi `mail()`: WordPress memakainya secara bawaan. Di Unix butuh program sendmail
  (binary kita bisa berperan sebagai itu lewat `sendmail_path`); di Windows cukup arahkan
  `SMTP` dan `smtp_port` di php.ini ke listener. Tanpa ini email WordPress di dev hilang.
- Kirim di production: jangan menjalankan MTA. Listener yang sama meneruskan ke penyedia
  SMTP lewat lettre, dengan batas laju dan log per situs.

Jadi satu komponen dengan dua mode: tangkap (dev) dan teruskan (production).

**8. Domain `*.test` dan HTTPS diremehkan.** Membuat sertifikat dengan rcgen adalah bagian
yang mudah. Yang berat: memasang root CA ke trust store tiap OS dan browser, resolusi DNS
tiap OS, mengikat port 80 dan 443, dan membuat PHP sendiri percaya pada CA itu. Pecah jadi
dua langkah:
- Langkah a: `namaproject.localhost`. Browser utama mengarahkannya ke loopback tanpa DNS dan
  tanpa hak admin. Ini bawaan.
- Langkah b: `.test` dan HTTPS sebagai pilihan, dengan satu kali izin admin. Kunci root CA
  dibuat per mesin, tidak pernah dikirim, dan dibatasi ke `.test` dan `.localhost`.

**9. Target fase 1 salah sasaran.** `laravel new` sejak Laravel 11 memakai SQLite secara
bawaan, jadi tidak butuh MariaDB. MariaDB baru diperlukan untuk WordPress, dan hanya di
desktop: di production, database adalah layanan terpisah di Dokploy.

**10. Lingkupnya terlalu lebar untuk tim kecil.** Tanpa pemangkasan, proyek ini berhenti di
tengah jalan. Dukungan `.htaccess` menambah lingkup, jadi pemangkasan lain makin penting.

**11. Server web "kecil" tetap harus benar soal keamanan.** Daftar wajibnya di bagian 8.

**12. Sikap v2 revisi 1 terhadap `.htaccess` salah untuk audiens ini.** Revisi 1 menyuruh
"jangan pernah mem-parse `.htaccess`". Itu cara berpikir hosting nginx. Audiens produk ini
datang dari XAMPP dan hosting cPanel, tempat `.htaccess` selalu berfungsi. Dikoreksi di
bagian 7.

### Kenapa tidak membungkus FrankenPHP saja?

| | Server sendiri (hyper + FastCGI) | Membungkus FrankenPHP |
|---|---|---|
| Kematangan | Baru, tanggung jawab sendiri | Sudah dipakai di production |
| Banyak versi PHP | Mudah: satu pool per versi | Satu versi per binary |
| Build PHP | NTS resmi | ZTS |
| `.htaccess` | Bisa dibuat (bagian 7) | Tidak ada, dan bukan milik kita untuk ditambah |
| HTTPS lokal | Kerjakan sendiri | Caddy sudah punya CA lokal dan pemasangan trust |
| Isolasi crash | Per proses | PHP satu proses dengan server |

Rekomendasi: server sendiri. Keputusan soal `.htaccess` memperkuat pilihan ini.

## 5. Arsitektur

```
        folder project (+ .htaccess, + jamu.toml bila ada)
                         |
        +----------------+----------------+
        |        core headless (Rust)     |
        |  - deteksi project              |
        |  - web server: hyper            |
        |  - lapisan .htaccess            |
        |  - pool PHP lewat FastCGI       |
        |  - worker + penjadwal           |
        |  - mail: tangkap / teruskan     |
        |  - API lokal + dashboard web    |
        +----------------+----------------+
            |                         |
   container production          desktop
   (Linux, PID 1,                (CLI, lalu tray Tauri yang
   di belakang Traefik)          hanya membuka dashboard)
```

| Komponen | Pilihan | Catatan |
|---|---|---|
| Web server | hyper | Di production cukup HTTP/1.1 karena TLS diakhiri Traefik. TLS lokal lewat rustls. |
| PHP, Linux dan macOS | php-fpm | Satu proses fpm per versi, lewat unix socket. |
| PHP, Windows | `php-cgi.exe` dengan `PHP_FCGI_CHILDREN` | Dikoreksi di revisi 9 (bagian 18): `PHP_FCGI_CHILDREN` berlaku di Windows. Satu proses induk di satu port loopback membuat pekerja, membagi satu soket ke semuanya, dan mengganti pekerja yang keluar setelah `PHP_FCGI_MAX_REQUESTS`. Supervisor hanya mengawasi induknya. |
| Sambungan keduanya | Daftar alamat upstream FastCGI | Web server hanya tahu daftar alamat. Cara menyalakan proses berbeda per OS, kode routing sama. |
| Supervisor | tokio | Windows: Job Object agar anak ikut mati. Unix: process group. Restart dengan jeda bertambah. Di container: PID 1, memanen zombie, meneruskan SIGTERM, menuntaskan request sebelum keluar. |
| Penjadwal | Bawaan core | Menjalankan `schedule:run` Laravel dan `wp-cron.php` tanpa cron sistem. |
| Mail | Listener SMTP + lettre | Mode tangkap dan mode teruskan. MailCrab sebagai referensi. |
| Database | MariaDB sebagai proses terkelola, hanya desktop | Diunduh saat dibutuhkan. Dimatikan dengan benar. Dump otomatis berkala (bagian 9, D4). |
| Dashboard | Web UI yang disajikan core | Tiga hal saja: Situs, Mail, Log. |
| GUI | Tauri, tipis | Ikon tray, webview ke dashboard, autostart, permintaan izin admin. |
| Distribusi komponen | Manifest (versi, URL, sha256) | Dicerminkan di server sendiri. MariaDB berlisensi GPL: sertakan tautan sumber. |

Keamanan API lokal: ikat ke 127.0.0.1, wajib token, periksa header `Host` dan `Origin`.
Tanpa itu daemon lokal bisa diserang dari halaman web mana pun yang dibuka developer.

### Rakit, jangan bangun: Redis, HTTP/3, multisite, worker

Ujiannya satu: apakah mahal bila ditambahkan belakangan? Yang murah ditunda. Yang mahal
diberi tempat di desain sekarang, walau pengerjaannya nanti. Keempat fitur ini bisa
dipenuhi tanpa membangun satu pun dari nol.

| Fitur | Siapa yang menyediakan | Yang harus ada di desain sejak awal | Kapan |
|---|---|---|---|
| Redis | Desktop: proses terkelola seperti MariaDB. Production: layanan terpisah di Dokploy. | Extension `redis` ada di paket PHP sejak rilis pertama. | Extension: fase 1. Server: fase 3. |
| HTTP/3 | Tepi jaringan: Traefik di production, Caddy di desktop bila HTTPS diaktifkan. | Tidak ada. Runtime cukup bicara HTTP/1.1 ke tepi. | Production: pengaturan Traefik, tanpa kode. |
| Multisite subdirektori | Lapisan `.htaccess` tahap 1. | Aturannya masuk korpus uji diferensial. | Fase 1. |
| Multisite subdomain | Runtime dan platform hosting. | Pencocokan host menerima wildcard (`*.situs.localhost`). | Runtime: fase 1. Domain dan sertifikat wildcard: pekerjaan platform. |
| Worker mode | Laravel Octane (FrankenPHP, RoadRunner, atau Swoole) sebagai proses terkelola. | Jenis upstream "reverse proxy HTTP" di samping FastCGI. | Setelah ada pelanggan Laravel yang meminta dan benchmark pada aplikasi nyata. |

Catatan per fitur:

- **Redis.** Server Redis tidak punya build Windows resmi. Pilihannya Memurai (komersial,
  ada edisi developer gratis), build tidak resmi, atau Garnet yang belum mendukung Streams.
  Ini satu-satunya penghambat, dan hanya di desktop Windows.
- **HTTP/3.** Traefik v3 mendukungnya per entrypoint. Dokploy sejak v0.20.0 mengaktifkannya
  untuk instalasi baru; instalasi lama perlu menyunting `traefik.yml` dan membuka UDP 443.
  Di loopback HTTP/3 tidak memberi manfaat yang terukur.
- **Multisite.** Aturan yang dihasilkan WordPress untuk multisite (`wp-admin/includes/network.php`)
  hanya memakai flag L, R, E, OR dan tes `-f`/`-d`. Semuanya ada di tahap 1.
- **Async dan coroutine (TrueAsync, Swoole).** Ini mengubah model eksekusi: satu pekerja
  melayani banyak permintaan yang sedang menunggu I/O. Manfaatnya nyata untuk aplikasi yang
  lama menunggu database atau API luar, karena satu pekerja php-fpm yang menunggu memakan
  puluhan MB sedangkan satu coroutine hanya beberapa KB. Tiga alasan ia tidak masuk
  sekarang. Pertama, TrueAsync adalah fork PHP (benchmark-nya memakai 8.6.0-dev ZTS) dan
  RFC-nya masih dibahas; JamuPHP memakai build PHP resmi. Kedua, WordPress tidak bisa
  memakainya, dan Laravel butuh adaptor pihak ketiga. Ketiga, pada beban yang kami ukur
  leher botolnya CPU, bukan I/O: 16 pekerja lebih lambat daripada 8. Tempatnya sudah ada
  di desain: server async dijalankan sebagai proses terkelola dan Caddy meneruskan
  permintaan ke sana, sama seperti Octane. Benchmark yang dikutip owner (989 lawan sekitar
  185 req/dtk pada 4 pekerja) berasal dari satu penulis dan belum diulang pihak lain.
- **Extension PHP di Rust (ext-php-rs) untuk hot path.** Ini alat bagi pembuat aplikasi
  yang sudah punya satu fungsi berat hasil profiling. Ia bukan fitur platform: hot path
  milik pelanggan ada di kode mereka, bukan di kode kita, dan beban WordPress tersebar di
  ribuan fungsi. Mengirim extension sendiri juga berarti membangunnya untuk tiap versi
  PHP, OS, dan arsitektur, yang bertentangan dengan keputusan memakai build PHP resmi.
  Urutan tuas yang terukur tetap: cache halaman, lalu jumlah pekerja yang tepat, lalu
  Redis, lalu worker mode, lalu async, lalu extension khusus.
- **Worker mode.** WordPress tidak bisa memakainya. Untuk Laravel, saya tidak menemukan
  benchmark yang teliti pada aplikasi nyata dengan database; angka yang beredar berasal
  dari vendor. Mode klasik FrankenPHP diukur Tideways setara dengan php-fpm. Di dev,
  worker mode merugikan: perubahan kode butuh reload dan state bocor antar request.

### Tepi desktop: Caddy sebagai kandidat

Di production, Traefik mengurus TLS dan HTTP/3 di depan runtime. Susunan yang sama bisa
dipakai di desktop: saat pengguna mengaktifkan HTTPS, Caddy diunduh sebagai komponen dan
ditaruh di depan runtime. CA lokal Caddy memasang dirinya ke trust store Windows, macOS,
Linux, dan Firefox, dan `caddy untrust` mencabutnya. Ini menggantikan rcgen dan kode trust
store buatan sendiri. Biayanya satu proses dan satu unduhan tambahan. Putuskan di awal
fase 3 dengan membandingkannya terhadap kode trust store untuk Windows saja.

Caddy sebagai lapisan tengah (pengganti server kita) tidak cocok: tidak ditemukan modul
`.htaccess` untuk Caddy, sehingga mesin `.htaccess` tetap harus ditulis, hanya saja dalam
Go sebagai modul Caddy.

### Pengukuran lapisan web (13 Nov 2026)

Pertanyaan owner: adakah pilihan yang lebih baik dari Caddy + php-fpm untuk performa
tinggi dan resource murah?

Cara ukur: WordPress 7.2-beta dengan database SQLite, PHP 8.5.12 dengan OPcache aktif di
semua susunan, server dibatasi 2 core, `wrk` 8 koneksi di 2 core lain. Memori adalah
jumlah PSS semua proses. Konfigurasi dan keluaran mentah ada di `/home/user/benchmark/`.

| Susunan | Memori diam | WordPress (req/dtk) | File statis 7 KB (req/dtk) | Halaman dari cache 82 KB (req/dtk) |
|---|---|---|---|---|
| nginx + php-fpm ondemand | 30 MB | 22,7 | 39.900 | 27.100 |
| Apache event + php-fpm ondemand | 32-36 MB | 23,1 | 19.500 | 14.100 |
| Caddy + php-fpm ondemand | 70-82 MB | 22,7 | 13.700 | 8.000 |
| nginx + php-fpm, 8 pekerja tetap | 117 MB | 22,7 | 39.900 | 27.100 |
| Apache mod_php, 8 pekerja tetap | 123 MB | 23,4 | 30.400 | 20.200 |
| Caddy + php-fpm, 8 pekerja tetap | 168 MB | 23,7 | 14.000 | 8.400 |
| FrankenPHP mode klasik, 8 thread | 182 MB | 21,7 | 13.700 | 8.000 |

Rincian proses saat diam: nginx 6 MB, Apache event 7-11 MB, Caddy 45-57 MB, master
php-fpm 23-25 MB. Permintaan pertama setelah pekerja ondemand habis: 0,08 detik, sama
dengan permintaan berikutnya.

Yang ditunjukkan angka ini:

1. **Web server tidak memengaruhi halaman dinamis.** Semua susunan menghasilkan 22-24
   req/dtk. Leher botolnya PHP.
2. **Cache halaman penuh adalah tuas terbesar.** Halaman yang sama dari cache dilayani
   350 sampai 1.200 kali lebih cepat daripada lewat PHP.
3. **Mode ondemand memangkas memori diam** dari 117 MB ke 30 MB per situs, tanpa
   penalti yang terukur pada permintaan pertama.
4. **Caddy adalah pilihan termahal di antara yang memakai php-fpm.** Sekitar 40-50 MB
   lebih banyak per situs dan sepertiga throughput statis nginx. Kelebihannya (HTTPS
   otomatis, HTTP/3) sudah disediakan Traefik di production.
5. **FrankenPHP mode klasik tidak lebih cepat dan paling boros memori** pada beban ini.
6. **Apache event + php-fpm ondemand hampir semurah nginx** dan `.htaccess` berfungsi
   penuh tanpa dibangun. Plugin cache WordPress yang menulis aturan `.htaccess` langsung
   bekerja, sehingga tuas nomor 2 didapat tanpa pekerjaan platform.

Per 100 situs diam: nginx sekitar 3 GB, Apache event 3,2-3,6 GB, Caddy 7-8 GB.

Batas pengukuran: satu kali jalan, tidak diulang. Mesin kecil dan `wrk` berjalan di mesin
yang sama. SQLite lewat lapisan penerjemah lebih lambat dari MySQL, jadi angka dinamis
absolutnya rendah; urutan antar susunan yang bisa dipegang, bukan angka absolutnya.
Memori Caddy bisa ditekan lewat pengaturan runtime Go dan itu tidak dicoba. Worker mode
tidak diukur karena WordPress tidak bisa memakainya. OpenLiteSpeed tidak diukur.

Diukur ulang di S4 (bagian 19) dengan MariaDB, cache halaman dari plugin, dan tiga kali
jalan. Satu dugaan di atas terkoreksi di sana: MariaDB hanya 11-15% lebih cepat dari
SQLite, jadi angka dinamis yang rendah bukan karena SQLite. Angka statis dan cache di
tabel ini juga tidak bisa dibandingkan langsung dengan bagian 19, karena di sini
permintaan lewat port yang dipublikasikan Docker dan di sana langsung ke IP container.

Rekomendasi pada titik ini (revisi 4) adalah Apache event + php-fpm ondemand. Rekomendasi
itu digantikan setelah syarat SSL bawaan dan uji konkurensi; lihat "Konkurensi" di bawah.

### SSL sah bawaan (13 Nov 2026)

Syarat owner: karena targetnya production, stack harus menerbitkan dan memperbarui
sertifikat sah sendiri, bukan menitipkannya ke komponen di luar stack.

**Satu fakta yang menentukan susunannya.** Pada satu alamat IP hanya satu proses yang bisa
memegang port 443. Jadi di server yang melayani banyak situs, penghenti TLS selalu satu
proses bersama, bukan satu per situs. Biaya memorinya dibayar sekali per server. Biaya per
situs tinggal sisi PHP.

Dua mode production:

| Mode | Siapa memegang 443 dan sertifikat | Lapisan per situs |
|---|---|---|
| Server mandiri (stack dipasang langsung di VPS) | Stack sendiri: Apache dengan mod_md | Pool php-fpm ondemand per situs |
| Di belakang proxy (Dokploy) | Traefik, yang sudah menerbitkan sertifikat sendiri | Container berisi Apache + php-fpm ondemand; mod_md mati |

Supervisor memilih mode secara otomatis: bila ia memegang port 443, mod_md aktif.

**Uji ujung ke ujung.** Sebuah CA ACME uji dijalankan lokal (protokol yang sama dengan
Let's Encrypt). Apache 2.4.69 dengan mod_md 2.6.10 dan Caddy 2.11.9 masing-masing diminta
melayani satu domain tanpa sertifikat awal.

| | Apache event + mod_md | Caddy |
|---|---|---|
| Sertifikat terbit otomatis | Ya, 1,2 detik | Ya, 0,9 detik |
| Rantai sertifikat lolos verifikasi klien | Ya | Ya |
| Aktif tanpa reload | Tidak. Perlu graceful reload; mod_md mengirim peristiwa `renewed` dan supervisor yang me-reload | Ya |
| Memori diam dengan TLS | 21 MB | 28 MB |
| Memori setelah beban | 32 MB | 36 MB |
| Halaman cache 82 KB lewat HTTPS | 6.500-6.800 req/dtk | 8.300 req/dtk |
| Koneksi baru dengan handshake TLS | 1.600 per detik | 1.800 per detik |
| HTTP/2 | Ya | Ya |
| HTTP/3 | Tidak | Ya |
| TLS on-demand (sertifikat saat domain pertama kali diakses) | Tidak | Ya |
| `.htaccess` | Ya | Tidak |

Keduanya memakai kunci ECDSA P-256 dan 2 core. Satu kali jalan.

**Koreksi atas revisi 4.** Pernyataan "Caddy pilihan termahal" berlaku untuk susunan satu
Caddy per container tanpa TLS. Bila server web menghentikan TLS sendiri dan dipakai
bersama semua situs, selisih memori Apache dan Caddy tinggal beberapa MB dan dibayar
sekali, dan Caddy sedikit lebih cepat lewat HTTPS. Memori Caddy yang terukur berkisar
28-57 MB tergantung beban.

**Rekomendasi pada titik ini (revisi 5)** adalah mulai dengan Apache + mod_md dan menaruh
Caddy di depan bila perlu. Urutan itu dibalik setelah uji konkurensi: Caddy menjadi server
terdepan sejak awal, dan Apache ditambahkan di belakangnya bila `.htaccess` diperlukan.
Lihat "Konkurensi" di bawah. Hasil uji mod_md tetap berlaku sebagai bukti bahwa jalur
Apache bisa dipakai bila owner menghendakinya.

**Yang harus ada agar SSL terasa tanpa config.**

- Menambah domain di dashboard adalah satu-satunya langkah. Supervisor menulis `MDomain`,
  me-reload, dan menampilkan status sertifikat.
- Sebelum meminta sertifikat, supervisor memeriksa bahwa DNS domain itu (A dan AAAA)
  mengarah ke server ini dan port 80 terjangkau. Bila belum, permintaan tidak dikirim.
- Penyimpanan sertifikat berada di volume yang bertahan saat container atau server diganti.
- Subdomain platform (misalnya `situs.namahosting.tld`) memakai satu sertifikat wildcard
  lewat DNS-01, bukan satu sertifikat per situs.
- Kunci ECDSA P-256. Dua tantangan didukung: HTTP-01 dan TLS-ALPN-01.
- Dashboard menampilkan sisa hari tiap sertifikat dan memberi peringatan bila perpanjangan
  gagal berulang. Let's Encrypt memendekkan masa berlaku ke 64 hari pada Februari 2027 dan
  45 hari pada Februari 2028, jadi perpanjangan otomatis harus bisa diandalkan. mod_md
  mendukung ARI (jadwal perpanjangan dari CA) secara bawaan.

### Konkurensi (13 Nov 2026)

Pertanyaan owner: bagaimana konkurensinya? Semua pengukuran sebelumnya hanya memakai 8
koneksi.

Konkurensi punya dua lapis yang berbeda sifatnya: jumlah koneksi yang dipegang web server,
dan jumlah eksekusi PHP yang berjalan bersamaan.

**Lapis 1: koneksi.** Halaman cache 82 KB, semua koneksi aktif terus-menerus, server
dibatasi 2 core. p99 adalah waktu respons untuk 1% permintaan terlambat.

| Server | 100 koneksi | 1.000 koneksi | 4.000 koneksi | Memori pada 4.000 |
|---|---|---|---|---|
| nginx (HTTP) | 65.400 req/dtk, p99 5 ms | 78.200 req/dtk, p99 32 ms | 62.300 req/dtk, p99 292 ms | 14 MB |
| Caddy (HTTP) | 10.100 req/dtk, p99 55 ms | 10.700 req/dtk, p99 455 ms | 10.200 req/dtk, p99 928 ms | 248 MB |
| Apache event, bawaan (HTTP) | 26.200 req/dtk, p99 196 ms | 24.800 req/dtk, p99 1,7 dtk | 24.200 req/dtk, p99 1,5 dtk, 37 timeout | 26 MB |
| Apache event, disetel (HTTP) | 23.200 req/dtk, p99 226 ms | 25.400 req/dtk, p99 2,4 dtk | 25.300 req/dtk, p99 2,5 dtk, 794 timeout | 93 MB |
| Caddy (HTTPS) | 13.400 req/dtk, p99 25 ms | 12.200 req/dtk, p99 154 ms | tidak diukur | 131 MB pada 1.000 |
| Apache event, disetel (HTTPS) | 9.000 req/dtk, p99 609 ms | 7.600 req/dtk, p99 3,7 dtk, koneksi terputus | tidak diukur | 250 MB pada 1.000 |

"Disetel" berarti 1.024 thread pekerja, keep-alive tanpa batas, dan antrean listen 4.096.
nginx dan Caddy tidak pernah menghasilkan error di uji ini.

**Lapis 2: eksekusi PHP.** Halaman depan WordPress dirender penuh, 2 core.

| Uji | nginx | Apache bawaan | Apache disetel | Caddy |
|---|---|---|---|---|
| 64 koneksi, 8 pekerja PHP | 28,1 req/dtk, median 2,2 dtk | 27,4 req/dtk, median 2,1 dtk | 28,5 req/dtk, median 2,0 dtk | 28,2 req/dtk, median 2,0 dtk |
| Halaman cache saat 300 koneksi menunggu PHP | 28.200 req/dtk | 0 req/dtk | 23.300 req/dtk | 5.900 req/dtk |

Dengan 16 pekerja PHP (Apache disetel, 64 koneksi): 23,8 req/dtk, median 2,6 dtk, memori
193 MB. Dengan 8 pekerja: 28,5 req/dtk dan 132 MB.

Yang ditunjukkan angka ini:

1. **Konkurensi PHP dibatasi jumlah core, bukan web server.** Dua core menghasilkan
   sekitar 28 halaman WordPress per detik di semua susunan. Pada 64 pengunjung bersamaan,
   masing-masing menunggu sekitar 2 detik di antrean.
2. **Menambah pekerja melebihi jumlah core memperlambat.** 16 pekerja lebih lambat 16% dan
   memakai memori 46% lebih banyak daripada 8 pekerja. Pekerja tambahan hanya berguna bila
   PHP banyak menunggu database atau API luar.
3. **Cache halaman adalah satu-satunya jalan ke konkurensi tinggi untuk WordPress.** Saat
   PHP kebanjiran, nginx dan Caddy tetap melayani ribuan halaman cache per detik.
4. **Apache dengan pengaturan bawaan lumpuh total saat PHP kebanjiran.** Semua 150 thread
   tersandera menunggu PHP, sehingga halaman cache dan file statis ikut berhenti.
5. **Apache tidak cocok sebagai server terdepan.** Pada ribuan koneksi, latensi ekornya
   1,5 sampai 3,7 detik dan ada timeout, juga setelah disetel. Lewat HTTPS pada 1.000
   koneksi ia kalah dari Caddy di throughput, latensi, memori, dan jumlah error.
6. **nginx paling kuat di semua ukuran koneksi**, dengan memori yang hampir tidak naik.
7. **Caddy stabil tanpa error, tetapi memorinya naik dengan jumlah koneksi**: 53 MB pada 8
   koneksi, 248 MB pada 4.000.

Batas pengukuran: semua koneksi dibuat aktif terus-menerus sampai CPU jenuh, dan itu
kasus terburuk. Pengunjung nyata sebagian besar diam di antara permintaan. Satu kali
jalan, 2 core, alat uji di mesin yang sama, HTTP/1.1 saja (browser memakai HTTP/2 dengan
satu koneksi per pengunjung). nginx lewat HTTPS dan HTTP/3 tidak diukur. Dua penyetelan
Apache dicoba; mungkin ada penyetelan lain yang lebih baik.

**Rekomendasi (revisi 6).**

| Tempat | Susunan | Alasan |
|---|---|---|
| Server mandiri, memegang port 443 | Caddy + pool php-fpm ondemand per situs | SSL sah bawaan yang matang dan tanpa reload, HTTP/3, tanpa error sampai 4.000 koneksi. Memorinya dibayar sekali per server. |
| Container per situs di belakang Traefik | nginx + php-fpm ondemand, atau Caddy demi keseragaman | TLS dan koneksi publik diurus Traefik. Di sini memori web server dibayar per situs: nginx 6 MB, Caddy 45-57 MB. |
| Desktop | Caddy + php-fpm (php-cgi di Windows) | CA lokal Caddy memasang diri ke trust store, dan build Windows resmi tersedia. |
| Situs yang butuh `.htaccess` | Apache di belakang Caddy | Apache hanya menerima sedikit koneksi dari Caddy, sehingga kelemahannya pada banyak koneksi tidak terpapar. |

nginx lebih cepat dan lebih hemat daripada Caddy di setiap uji koneksi. Ia tidak dipilih
untuk server mandiri karena modul ACME bawaannya masih muda (v0.4.1) dan tidak diuji di
sini, tidak punya CA lokal untuk desktop, dan build Windows-nya terbatas. Bila modul ACME
nginx terbukti andal, nginx layak dievaluasi ulang untuk semua tempat.

**Aturan konkurensi yang dihasilkan supervisor.**

- Jumlah pekerja PHP per situs mengikuti jatah core situs itu, bukan angka tetap. Bawaan:
  dua kali jumlah core, dan lebih bila situs terbukti banyak menunggu I/O.
- Antrean PHP per situs dibatasi. Permintaan di luar batas dijawab 503 dengan `Retry-After`
  dalam hitungan detik, bukan menunggu puluhan detik.
- Satu situs yang kebanjiran tidak boleh menghabiskan CPU situs lain: jatah CPU per situs.
- Batas memori untuk Caddy dipasang, karena memorinya naik dengan jumlah koneksi.
- Cache halaman aktif secara bawaan untuk pengunjung yang tidak login.

## 6. Tanpa config: cara kerja untuk pemula

Ukuran keberhasilannya: orang yang belum pernah mendengar kata "Apache" bisa menjalankan
WordPress. Panel XAMPP adalah contoh sebaliknya, karena pengguna harus tahu apa itu Apache
dan MySQL lalu menekan Start satu per satu.

### Prinsip

1. **Folder adalah situs.** Ada satu folder Situs. Setiap subfolder otomatis menjadi
   `namafolder.localhost`. Folder di tempat lain bisa ditambahkan dengan seret-lepas.
2. **Tidak ada tombol Start/Stop per layanan.** Aplikasi terbuka berarti semua hidup.
   Aplikasi ditutup berarti semua berhenti dengan rapi.
3. **Pengguna tidak pernah mengetik URL atau port.** Dashboard selalu memberi tautan yang benar.
4. **Dasar-dasarnya jalan tanpa hak admin.** `.test` dan HTTPS adalah pilihan tambahan.
5. **UI menulis ke file yang sama.** Mengganti versi PHP di dashboard menulis satu baris ke
   `jamu.toml` di folder project. Tidak ada pengaturan tersembunyi di tempat lain, sehingga
   pilihan itu ikut ke production lewat repo.
6. **Setiap kegagalan berupa satu kalimat dan satu tombol.** Apa yang terjadi, lalu cara
   memperbaikinya. Tidak ada kode error tanpa penjelasan.

### Deteksi otomatis

| Yang ditentukan | Aturan |
|---|---|
| Document root | Subfolder pertama dari `public`, `web`, `public_html`, `htdocs` yang berisi `index.php` atau `index.html`. Bila tidak ada, folder itu sendiri. |
| Jenis | Ada `artisan`: Laravel. Ada `wp-config.php` atau `wp-load.php`: WordPress. Selain itu: PHP umum. |
| Routing | Ada `.htaccess`: dipatuhi. Tidak ada: aturan bawaan sesuai jenis. |
| Versi PHP | Dari `require.php` di `composer.json`: versi terbaru yang memenuhi. Tidak ada: versi stabil terbaru. |
| Extension | Di dev semua extension bawaan aktif. Tidak ada daftar yang harus diisi. |
| Mail | Semua `mail()` otomatis masuk inbox lokal. Untuk Laravel, dashboard menyediakan tiga baris `.env` siap salin; `.env` pengguna tidak diubah diam-diam. |

Kode deteksi ini sama di laptop dan di container, sehingga hasilnya sama di kedua tempat.

### `jamu.toml` hanya untuk menimpa bawaan

```toml
# Semua baris opsional. File ini boleh tidak ada.
php  = "8.4"
root = "public"
scheduler = true

[ini]
memory_limit = "512M"
"date.timezone" = "Asia/Jakarta"

[[worker]]
name = "queue"
run  = "php artisan queue:work --tries=3"
```

Contoh di revisi sebelumnya memakai `php = "8.2"` bersama tabel `[php.ini]`. Itu bukan TOML
yang sah: `php` tidak bisa sekaligus teks dan tabel. Tabelnya bernama `[ini]`. Arti tiap
kunci, dan apa yang ditolak, ada di bagian 20.

Yang berbeda antar lingkungan (kredensial database, tujuan mail, domain) datang dari
environment variable, tidak pernah dari file ini. Nilai bawaan PHP (batas memori, ukuran
upload) sama di dev dan production, agar tidak ada situs yang jalan di laptop lalu mati
di hosting karena batasnya berbeda.

### Dua tombol yang menggantikan tutorial

- "Buat situs WordPress": mengunduh WordPress, membuat database, menulis `wp-config.php`,
  lalu membuka browser ke situs yang sudah jadi.
- "Buat situs Laravel": menjalankan `composer create-project`, lalu membuka browser.

## 7. Kompatibilitas Apache (`.htaccess`)

### Bukti dari kode sumber (dibaca 13 Nov 2026)

- **WordPress hanya menulis `.htaccess` bila server mengaku Apache.** Di
  `wp-includes/vars.php`, `$is_apache` bernilai benar bila `SERVER_SOFTWARE` memuat kata
  "Apache" atau "LiteSpeed". WP Rocket, W3 Total Cache, dan WP Super Cache bercabang pada
  variabel yang sama. Server yang tidak dikenali sebagai Apache tidak akan menerima aturan
  dari plugin-plugin ini.
- **Plugin memakai `.htaccess` untuk melindungi file.** Contact Form 7 menulis
  `Require all denied` dan `Deny from all` ke folder upload-nya. Di hosting tanpa dukungan
  `.htaccess`, perlindungan itu diam-diam tidak berlaku. Jadi mendukungnya adalah fitur
  keamanan, bukan hanya kenyamanan.
- **Kosakata yang dipakai sempit.** Hitungan kasar dengan grep atas 7 plugin dan
  konfigurasi h5bp (bukan sensus): hampir semuanya adalah RewriteCond/RewriteRule, Header,
  Expires, AddType, kontrol akses (Order/Deny/Allow/Require), dan blok Files/FilesMatch.
  Flag RewriteRule yang muncul hanya L, E, R, F, NC. Tes file hanya `-f` dan `-d`. Tidak
  ada RewriteMap, Auth, blok `<If>`, atau lookaround di sampel ini.
- **`php_value` tidak punya jalur di Windows.** php-fpm membaca parameter FastCGI
  `PHP_VALUE`; php-cgi tidak. Keduanya membaca `.user.ini`.

### Tiga pilihan, dinilai jujur

| | Apache asli sebagai lapisan web | Lapisan kompatibilitas di Rust | Tanpa dukungan |
|---|---|---|---|
| Kompatibilitas | 100% sejak hari pertama | Sebagian besar; tidak akan pernah 100% | Nol |
| Biaya | Kecil | Besar, setara web server itu sendiri | Nol |
| Satu binary, "berbasis Rust" | Tidak | Ya | Ya |
| Windows | Bergantung build pihak ketiga | Kode yang sama | - |

Rekomendasi: lapisan kompatibilitas di Rust, karena kosakatanya sempit dan produk tetap
satu binary. Apache asli adalah jalan keluar yang sah bila spike S4 gagal. Ini juga jalur
dengan risiko terendah bila "web server ditulis di Rust" ternyata bukan harga mati.

### Rancangan

**Identitas server.** `SERVER_SOFTWARE` memuat kata "Apache", misalnya
`Apache (kompatibel) stack/1.0`. Tanpa ini WordPress dan plugin tidak menulis aturannya.
Konsekuensinya: plugin akan menganggap kita Apache penuh, jadi dua aturan berikut wajib.

**`<IfModule>` sebagai katup pengaman.** Jawab benar hanya untuk modul yang directive-nya
kita jalankan (rewrite, headers, expires, mime, dir, alias, setenvif, authz). Jawab salah
untuk sisanya, termasuk mod_php, mod_security, dan LiteSpeed. Karena plugin membungkus
aturannya dengan `<IfModule>`, bagian yang tidak kita dukung terlewati dengan bersih,
persis seperti di Apache yang tidak memuat modul itu.

**Tidak pernah mengabaikan dalam diam.**
- Directive yang dikenal tapi belum didukung: situs tetap jalan, peringatan muncul di
  dashboard dengan nama file dan nomor baris.
- Directive kontrol akses yang tidak bisa dievaluasi penuh: akses ke lingkup itu ditolak
  (403). Mengabaikan "tolak" adalah satu-satunya kegagalan yang membocorkan data.
- Salah sintaks: Apache memberi 500. Kita juga, tetapi di dev halamannya menunjuk baris
  yang salah.

**Cakupan bertahap.**

| Tahap | Isi |
|---|---|
| 1 (fase 1) | mod_rewrite konteks direktori: RewriteEngine, RewriteBase, RewriteCond (variabel server umum, `-f -d -s -l`, `!`, NC, OR), RewriteRule (L, END, R, F, G, QSA, QSD, NC, NE, E, T, S, C). Kontrol akses: Require all/ip, Order/Deny/Allow. Files, FilesMatch, IfModule, DirectoryIndex, ErrorDocument, Redirect, RedirectMatch, Options. |
| 2 (fase 2) | Header, RequestHeader, Expires*, AddType, AddCharset, AddEncoding, FileETag, SetEnv, SetEnvIf, BrowserMatch. Directive kompresi dipetakan ke kompresi bawaan server. |
| 3 (bila diminta) | Basic auth (ditolak sampai tersedia), blok `<If>`, pemetaan `php_value`. |
| Tidak akan | RewriteMap, flag proxy `[P]`, handler CGI, SSI, mod_security, daftar isi direktori. |

**Kekhasan mod_rewrite yang harus persis sama.** Pemotongan dan penambahan kembali
prefiks direktori. Aturan diproses ulang dari atas setelah rewrite, sehingga `[L]` bukan
akhir dan batas putaran 10 kali diperlukan. Aturan rewrite di subfolder menggantikan milik
folder induk, bukan menambah. Redirect garis miring di akhir nama direktori. Pola memakai
sintaks PCRE, jadi butuh mesin regex yang mendukung lookaround dan backreference, dengan
batas backtracking.

**Kinerja.** Hasil parse disimpan per direktori dan dibaca ulang hanya bila waktu ubah
file berganti. Perubahan oleh plugin langsung berlaku tanpa restart.

**Pengujian diferensial terhadap Apache asli.** Korpus `.htaccess` dari WordPress,
Laravel, CodeIgniter, Drupal, Joomla, h5bp, dan plugin populer dijalankan lewat Apache
httpd di Docker dan lewat server kita, dengan daftar request yang sama. Status, header
Location, file atau skrip yang dipilih, dan header respons dibandingkan. Setiap selisih
adalah bug atau pengecualian yang ditulis di halaman kompatibilitas. Tanpa pengujian ini,
klaim "kompatibel" tidak punya dasar.

## 8. Daftar wajib web server (tulis tesnya dulu)

- Normalisasi path; tolak `..` dan byte nol.
- Tolak dotfile (`.env`, `.git`, `.htaccess`, `.user.ini`), kecuali `.well-known`.
- Hanya jalankan file `.php` yang benar-benar ada di disk. `SCRIPT_FILENAME` ditentukan
  server, bukan diambil dari URL. Ini menutup celah jenis `/uploads/x.jpg/y.php`.
- Aturan bawaan Laravel (bila tanpa `.htaccess`): hanya `public/index.php` yang dijalankan.
- Aturan bawaan WordPress: file `.php` yang ada boleh jalan, `index.php` sebagai indeks
  direktori, sisanya jatuh ke `/index.php`. PHP di `wp-content/uploads` ditolak.
- Windows: `index.php::$DATA`, titik atau spasi di akhir nama, beda huruf besar-kecil, dan
  nama pendek 8.3 tidak boleh membuat kode sumber PHP tersaji sebagai file statis.
- Buang header `Proxy` (httpoxy). `HTTPS` dan `REMOTE_ADDR` hanya dipercaya dari proxy
  yang didaftarkan.
- Body request dan respons dialirkan, tidak ditampung utuh. Ada batas ukuran dan timeout.
- File statis: ETag, Range, MIME, kompresi.

## 9. Jalur negatif

Untuk tiap situasi: apa yang terjadi bila dibiarkan, dan perilaku yang dirancang.
Yang bertanda (!) paling sering menimpa pemula atau paling mahal akibatnya.

### A. Pemasangan dan start pertama

| # | Situasi | Bila dibiarkan | Perilaku yang dirancang |
|---|---|---|---|
| A1 (!) | Antivirus atau SmartScreen memblokir installer, atau mengarantina `php-cgi.exe` | Aplikasi "rusak" tanpa penjelasan | Installer bertanda tangan. Saat start, tiap komponen diperiksa keberadaan dan hash-nya. Bila hilang: "Antivirus menghapus PHP" dengan tombol Pulihkan. |
| A2 (!) | Tidak punya hak admin (laptop kampus atau kantor) | Pemasangan gagal | Pasang per pengguna. `*.localhost` dan port cadangan tidak butuh admin. Fitur yang butuh admin menyatakannya sebelum diklik. |
| A3 (!) | Tanpa internet, atau unduhan putus di tengah | Komponen setengah terpasang | Tersedia installer lengkap berisi satu PHP dan MariaDB. Unduhan bisa dilanjutkan, diverifikasi sha256, dan baru dipindah ke tempatnya setelah utuh. |
| A4 (!) | Port 80, 443, atau 3306 dipakai XAMPP, Laragon, IIS, atau MySQL lain | "Port already in use", keluhan terbanyak pengguna XAMPP | Deteksi, sebutkan nama program pemakainya, pindah otomatis ke port lain. Tautan di dashboard ikut menyesuaikan. |
| A5 | Nama pengguna Windows berisi spasi atau huruf non-ASCII; path lebih dari 260 karakter | Gagal di tempat acak | Semua path diperlakukan sebagai Unicode. CI punya akun uji bernama seperti `Budi Santoso é`. |
| A6 (!) | Folder situs atau data berada di OneDrive atau Dropbox | Database rusak karena disinkronkan saat ditulis | Data database selalu di folder lokal aplikasi, tidak pernah di folder tersinkron. Situs di folder tersinkron diberi peringatan. |
| A7 | Dua salinan aplikasi berjalan, atau proses sisa dari crash sebelumnya | Port bentrok dengan diri sendiri | Kunci satu instance. Saat start, proses yatim yang tercatat dimatikan. |

### B. Membuat dan membuka situs

| # | Situasi | Bila dibiarkan | Perilaku yang dirancang |
|---|---|---|---|
| B1 | Deteksi document root atau jenis project salah | Halaman kosong atau 404 | Dashboard menampilkan hasil deteksi dan bisa diubah satu klik. Pilihan tersimpan di `jamu.toml`. |
| B2 | Versi PHP yang diminta `composer.json` belum terpasang | Error Composer yang tidak dimengerti pemula | Unduh otomatis dengan indikator kemajuan. Bila tanpa internet: pakai versi terdekat dan tampilkan pemberitahuan. |
| B3 | Extension yang dibutuhkan tidak ada di paket (misalnya ioncube, oci8) | Fatal error PHP | Pesan "Extension X tidak tersedia di paket ini" beserta daftar yang tersedia. |
| B4 | Nama folder berisi spasi, garis bawah, atau huruf besar | Hostname tidak sah | Nama diubah otomatis menjadi bentuk yang sah; bentrokan diberi akhiran. Pemetaannya terlihat di dashboard. |
| B5 | `*.localhost` tidak dikenali suatu alat atau browser | Situs tidak bisa dibuka | Setiap situs juga punya port tetap sendiri sebagai jalur cadangan (`127.0.0.1:port`). Perlu diuji per OS. |
| B6 (!) | WordPress memanggil dirinya sendiri (wp-cron, Site Health) saat semua pekerja PHP sibuk | Macet total. Ini masalah klasik server satu proses. | Pool minimal 4 pekerja per versi, antrean dengan batas waktu, dan pesan "semua pekerja PHP sibuk". |

### C. `.htaccess`

| # | Situasi | Bila dibiarkan | Perilaku yang dirancang |
|---|---|---|---|
| C1 (!) | Directive kontrol akses tidak didukung | File pribadi terbuka ke publik | Akses ditolak untuk lingkup itu, dengan peringatan. |
| C2 | Directive lain tidak didukung | Fitur plugin diam-diam mati | Situs jalan; peringatan dengan file dan nomor baris. |
| C3 | Salah sintaks | 500 tanpa petunjuk | Di dev: halaman yang menunjuk barisnya. Di production: 500 dan log. |
| C4 | Putaran rewrite tanpa akhir | Pekerja habis | Berhenti di 10 putaran dengan penjelasan. |
| C5 | Pola regex yang meledak | Pekerja tergantung | Batas backtracking; request itu gagal, server tetap sehat. |
| C6 | Plugin menganggap kita Apache penuh lalu menulis aturan eksotis | Perilaku berbeda dari hosting asal | `<IfModule>` menjawab salah untuk modul yang tidak ada. Plugin populer masuk korpus uji. |
| C7 | `php_value` di luar `<IfModule>` | Di Apache dengan FPM ini pun menghasilkan 500 | Peringatan, dengan tawaran memindahkannya ke `[php.ini]` di `jamu.toml`. |
| C8 (!) | Jalan di laptop Windows, 404 di production Linux karena beda huruf besar-kecil | Baru ketahuan setelah deploy | Perintah `check` sebelum deploy menandai path yang hanya cocok bila huruf besar-kecil diabaikan. |
| C9 | Situs dibobol lalu `.htaccess` diubah untuk redirect spam | Sama seperti di Apache | Di hosting: pemberitahuan saat `.htaccess` berubah. Fitur lanjutan. |

### D. Saat berjalan

| # | Situasi | Bila dibiarkan | Perilaku yang dirancang |
|---|---|---|---|
| D1 | Fatal error PHP, layar putih | Pemula tidak tahu harus melihat ke mana | Di dev error ditampilkan, dan halaman error menaut ke tab Log pada baris yang tepat. |
| D2 | `php-cgi` crash atau keluar setelah batas request | Request gagal acak | Dihidupkan ulang tanpa terlihat. Bila crash berulang: jeda bertambah dan stderr ditampilkan. |
| D3 | Request sangat lama | Pekerja tersandera | Batas waktu yang sama di dev dan production, dengan pesan yang menyebut batasnya. |
| D4 (!) | MariaDB tidak mau hidup setelah listrik padam atau laptop mati paksa | "MySQL shutdown unexpectedly" di XAMPP: data hilang, pengguna panik | Pemulihan crash otomatis. Bila gagal: folder data tidak disentuh, dan ditawarkan pemulihan dari dump otomatis. Dump tiap database dibuat berkala dan saat aplikasi ditutup. |
| D5 | Disk penuh | Unduhan atau database rusak | Ruang diperiksa sebelum mengunduh; pesan yang jelas. |
| D6 | VPN aktif atau jaringan berganti | DNS `.test` bisa putus | `*.localhost` tidak terpengaruh. Ini alasan ia dijadikan bawaan. |
| D7 | Pengguna menunggu email sungguhan tiba | Mengira fitur email rusak | Tulisan tetap di inbox: "Email tidak dikirim ke luar. Semua tertangkap di sini." |

### E. Pembaruan dan pencopotan

| # | Situasi | Bila dibiarkan | Perilaku yang dirancang |
|---|---|---|---|
| E1 | Pembaruan aplikasi gagal di tengah | Aplikasi tidak bisa dibuka | Pergantian atomik dengan versi lama disimpan untuk kembali. |
| E2 | Versi MariaDB naik | Data tidak terbaca | Dump otomatis sebelum naik versi. |
| E3 | Rilis keamanan PHP | Pengguna bertahan di versi rentan | Pemberitahuan dan satu klik. Versi lama dihapus setelah yang baru terbukti jalan. |
| E4 (!) | Aplikasi dicopot | Root CA tepercaya dan aturan DNS tertinggal, dan itu lubang keamanan | Pencopot menghapus root CA, aturan DNS, dan layanan. Situs dan database dipertahankan kecuali diminta dihapus. |
| E5 | Pengguna menolak dialog pemasangan root CA | Peringatan sertifikat terus-menerus | HTTPS tetap mati, HTTP tetap jalan, tidak ada desakan berulang. |

### F. Production

| # | Situasi | Bila dibiarkan | Perilaku yang dirancang |
|---|---|---|---|
| F1 (!) | Deploy membawa directive kontrol akses yang tidak didukung | Sama dengan C1, tetapi di internet | `check` menggagalkan deploy. |
| F2 (!) | Bug di runtime kita mengenai semua situs sekaligus | Insiden seluruh platform | Versi runtime dikunci per situs, peluncuran bertahap, bisa kembali ke versi lama. |
| F3 | Container kehabisan memori | Situs mati-hidup tanpa sebab yang terlihat | Healthcheck, restart, dan penyebabnya tercatat di log. |
| F4 | Situs yang dibobol mengirim spam | Reputasi IP hosting rusak | Batas laju dan log per situs pada mode teruskan. |

### G. SSL di production

| # | Situasi | Bila dibiarkan | Perilaku yang dirancang |
|---|---|---|---|
| G1 (!) | Domain ditambahkan sebelum DNS mengarah ke server | Validasi gagal berulang lalu kena batas laju CA; domain terkunci berjam-jam | Periksa DNS dulu. Tampilkan "DNS belum mengarah ke server ini" beserta nilai yang diharapkan. Coba lagi otomatis dengan jeda bertambah. |
| G2 (!) | Penyimpanan sertifikat hilang saat container atau server diganti | Semua sertifikat diminta ulang sekaligus dan kena batas laju | Penyimpanan di volume yang bertahan, ikut dicadangkan. |
| G3 (!) | Perpanjangan gagal diam-diam | Situs tiba-tiba tidak bisa dibuka saat sertifikat kedaluwarsa | Sisa hari tampil di dashboard. Peringatan dikirim jauh sebelum kedaluwarsa. |
| G4 | Sertifikat baru sudah terbit tapi belum aktif | Sertifikat lama dipakai sampai kedaluwarsa | Supervisor menangkap peristiwa `renewed` dan melakukan graceful reload. Terverifikasi di uji. |
| G5 | Sertifikat belum ada saat domain baru diakses lewat HTTPS | Apache menjawab 503 untuk domain itu | HTTP tetap dilayani. Pengalihan ke HTTPS dan HSTS baru dinyalakan setelah sertifikat aktif. |
| G6 | Rekaman AAAA (IPv6) lama mengarah ke server lain | Validasi gagal walau rekaman A benar | Pemeriksaan awal mencakup AAAA dan menyebutkan rekaman mana yang salah. |
| G7 | Domain memakai proxy Cloudflare | TLS-ALPN-01 gagal | HTTP-01 dipakai; keduanya tersedia. |
| G8 | Rekaman CAA melarang CA yang dipakai | Penolakan yang membingungkan | Pesan yang menyebut rekaman CAA sebagai penyebabnya. |
| G9 | CA utama sedang gangguan | Domain baru tidak mendapat sertifikat | CA cadangan dikonfigurasi. Perlu diverifikasi di mod_md. |
| G10 | Banyak situs memakai subdomain platform | Kena batas jumlah sertifikat per domain terdaftar | Satu sertifikat wildcard lewat DNS-01. |
| G11 | Ribuan domain dalam satu Apache | Reload makin lambat | Saat itu terjadi, pindah ke tahap 2 (Caddy di depan dengan TLS on-demand). |

### H. Konkurensi

| # | Situasi | Bila dibiarkan | Perilaku yang dirancang |
|---|---|---|---|
| H1 (!) | Lonjakan pengunjung ke halaman yang tidak di-cache | Antrean PHP memanjang, semua pengunjung menunggu puluhan detik lalu timeout | Antrean per situs dibatasi; kelebihan dijawab 503 dengan `Retry-After`. Cache halaman aktif secara bawaan. |
| H2 (!) | Jumlah pekerja PHP diisi terlalu besar "supaya kuat" | Lebih lambat dan memori habis; terukur 16% lebih lambat dan 46% lebih boros | Supervisor menghitungnya dari jatah core. Tidak ada kolom angka bebas di dashboard. |
| H3 (!) | Satu situs kebanjiran di server bersama | Situs lain ikut lambat | Jatah CPU dan pool PHP per situs. |
| H4 (!) | Apache dipakai sebagai server terdepan dengan pengaturan bawaan | Saat PHP kebanjiran, halaman cache dan file statis ikut berhenti (terukur 0 req/dtk) | Apache hanya di belakang Caddy. Bila tetap dipakai di depan, jumlah thread harus jauh di atas batas antrean PHP. |
| H5 | Ribuan koneksi terbuka bersamaan | Memori Caddy naik (terukur 248 MB pada 4.000 koneksi) | Batas memori dan batas koneksi dipasang; penggunaan tampil di dashboard. |
| H6 | Klien lambat menahan koneksi tanpa mengirim apa-apa | Slot koneksi habis | Batas waktu baca header dan body. |
| H7 | Plugin cache tidak terpasang atau salah atur | Setiap pengunjung menjalankan PHP | Direvisi di bagian 19: platform tidak membuat cache sendiri. Plugin cache dipasang oleh hosting saat situs dibuat, dan `jamu` menyajikan berkas cache buatan plugin langsung dari Caddy. |
| H8 (!) | PHP kebanjiran permintaan yang tidak bisa di-cache (tebak sandi di `wp-login.php`, bot dengan query acak), plugin cache dalam mode yang menyajikan lewat PHP | Halaman yang sudah di-cache ikut antre di belakang banjir; terukur 0,5 req/dtk dengan waktu tunggu 12 detik | Berkas cache disajikan Caddy tanpa menyentuh PHP; terukur 4.800 req/dtk pada banjir yang sama. |
| H9 (!) | Folder plugin cache dihapus tanpa dinonaktifkan, sementara berkas cache masih ada | Dengan aturan berkas cache, Caddy terus menyajikan halaman basi tanpa batas waktu (terukur). Tanpa aturan, WordPress kembali menyajikan halaman segar. | Aturan hanya berlaku selama berkas plugin masih ada di tempatnya; belum dibuat. |
| H10 | Situs HTTPS di belakang proxy, aturan cache hanya mengenal `index.html` | Plugin menyimpan halaman HTTPS di `index-https.html`: aturan tidak pernah kena, atau menyajikan varian HTTP ke pengunjung HTTPS | Dua aturan, satu per skema (sudah di kandidat aturan, teruji). |

## 10. Roadmap dan syarat lulus

Ukuran: K kecil, S sedang, B besar. Ini perkiraan kasar, bukan janji waktu.

**Fase 0, spike (S).** Membuktikan asumsi paling berisiko dengan kode sekali buang.
- S1: supervisor Rust menyalakan Caddy dan php-fpm ondemand di Linux dari hasil deteksi
  project. Laravel baru dan instalasi WordPress jalan, termasuk upload besar, dengan
  sertifikat dari staging Let's Encrypt pada domain sungguhan.
- S2: hal yang sama di Windows dengan Caddy dan pool php-cgi dari zip resmi.
- S3: image `php:*-fpm` + binary sebagai PID 1, satu situs sungguhan di Dokploy.
- S4: ulangi pengukuran bagian 5 dengan MySQL dan plugin cache halaman, tiga kali jalan
  (sudah, bagian 19).
  Hanya bila owner memilih server sendiri: `.htaccess` untuk empat kasus (WordPress,
  Laravel, blok tolak Contact Form 7, aturan WP Super Cache), diuji diferensial terhadap
  Apache.
- Lulus bila keempatnya jalan. Bila S2 macet di Windows, evaluasi nginx untuk Windows atau
  FrankenPHP sebagai lapisan web desktop.

**Fase 1, core headless (B).** Deteksi project, perintah `serve`, `jamu.toml`, pekerja,
penjadwal, log, shutdown yang rapi, penerus `php` dan `composer`. Sedang dikerjakan;
keadaannya di bagian 20.
- Lulus bila `composer create-project laravel/laravel` lalu `serve` jalan tanpa file config
  di Linux dan Windows, dan tes bagian 8 hijau di kedua OS.

Rumusan awal fase ini ditulis sebelum keputusan Caddy. Yang berubah:

| Butir rumusan awal | Sekarang | Alasan |
|---|---|---|
| `.htaccess` tahap 1 dan uji diferensial terhadap Apache | Dicoret dari fase 1 | Owner menunda `.htaccess`; tanpa itu tidak ada yang dibandingkan dengan Apache. |
| Pengelolaan pool PHP | Tidak dibangun | php-fpm dan `php-cgi` sudah mengurus pekerjanya sendiri (bagian 18). |
| Pencocokan host dengan wildcard, upstream reverse proxy HTTP | Tidak dibangun | Keduanya kemampuan bawaan Caddy. Baru disentuh saat ada fitur yang memakainya. |
| Extension `redis` | Sudah ada di image (bagian 17) | Untuk rakitan Windows menunggu fase 3; zip resmi PHP tidak membawanya. |
| Aturan multisite WordPress di korpus uji | Pindah ke fase 2 | Satu kelompok dengan pekerjaan WordPress production lain: wp-cron dan aturan berkas cache. |
| Penerus `php` dan `composer` yang memilih versi | Penerus sudah ada; memilih versi menunggu fase 3 | Baru bermakna bila ada lebih dari satu versi PHP terpasang, dan itu pekerjaan "unduh dan ganti versi PHP". Sampai saat itu `php = "8.4"` adalah janji yang diperiksa, bukan pemilih. |

**Fase 2, production (S).** Sedang dikerjakan; keadaannya di bagian 21. Dua mode: server mandiri dengan SSL sah bawaan Caddy (tambah
domain, pemeriksaan DNS, status sertifikat), dan container di belakang Traefik. Lalu mail
mode teruskan, wp-cron, aturan multisite WordPress, aturan berkas cache buatan plugin
(bagian 19), log akses, log JSON, perintah `check`. Healthcheck, kepercayaan proxy, dan
non-root sudah ada sejak S3. Uji SSL terhadap staging Let's Encrypt dengan domain
sungguhan sebelum dipakai pelanggan.
- Urutan pemakaian: situs internal, lalu beberapa pelanggan, lalu bawaan.
- Lulus bila sejumlah situs berjalan 30 hari tanpa insiden yang disebabkan runtime.

**Fase 3, desktop Windows (B).** Installer, folder Situs, dashboard tiga tab, unduh dan
ganti versi PHP, MariaDB terkelola dengan dump otomatis, Redis sebagai komponen pilihan,
inbox mail, tray, `.test` + HTTPS sebagai pilihan (lewat Caddy atau kode sendiri),
pembaruan otomatis, code signing, dan seluruh jalur negatif bagian A sampai E.
- Lulus bila di VM Windows 11 yang bersih, tanpa hak admin, dari instal sampai satu situs
  WordPress dan satu situs Laravel jalan butuh kurang dari 5 menit tanpa membuka terminal
  dan tanpa mengisi satu pun pengaturan. Lalu ulangi dengan XAMPP sedang berjalan dan
  dengan internet dimatikan.

**Fase 4, pembeda (S).** Perintah tarik dan dorong situs antara laptop dan hosting.

**Fase 5, macOS (S).** Butuh build PHP sendiri dan notarization Apple.

### Tidak dikerjakan

Embed SAPI, worker mode buatan sendiri, HTTP/3 di dalam runtime, Pingora, FTP, bundel
Node, PostgreSQL/MongoDB, metrik Prometheus, daftar isi direktori, tombol Start/Stop per
layanan. Masing-masing baru dibuka kembali bila ada permintaan nyata dari pengguna.

Redis, HTTP/3 di tepi, multisite, dan worker mode lewat Octane tidak termasuk daftar ini;
lihat bagian 5.

## 11. Urutan OS: Linux, Windows, macOS

- Linux lebih dulu karena itu target production. Hasil fase 1 langsung terpakai di fase 2.
- Windows tidak boleh menjadi "port belakangan". Perbedaannya arsitektural: tidak ada
  php-fpm, tidak ada sinyal Unix, Job Object, aturan path yang lain. Build Windows dan
  smoke test php-cgi masuk CI sejak minggu pertama, walau rilis desktop baru di fase 3.
- macOS terakhir: port-nya murah karena mirip Linux, pesaingnya (Herd) paling kuat di
  sana, dan hanya di sana kita terpaksa membangun PHP sendiri.

## 12. Risiko

1. **Ekor kompatibilitas `.htaccess`.** Risiko baru dan yang terbesar dari sisi biaya.
   Tidak akan pernah 100%, dan setiap selisih adalah laporan bug. Diredam dengan cakupan
   bertahap, uji diferensial, halaman kompatibilitas, dan jalan keluar ke Apache asli.
2. **Windows.** Pengelolaan pool php-cgi, penghentian proses, dan keanehan filesystem.
   Diredam oleh spike S2 dan CI sejak awal.
3. **Code signing.** Azure Artifact Signing hanya menerima organisasi di AS, Kanada, Uni
   Eropa, dan Inggris, serta perorangan di AS dan Kanada. Badan usaha Indonesia kemungkinan
   besar harus membeli sertifikat OV dari CA komersial. Alat yang mengubah DNS, memasang
   root CA, dan menjalankan server berperilaku mirip malware di mata antivirus.
4. **Server buatan sendiri di production.** Diredam dengan berada di belakang Traefik,
   parsing HTTP oleh hyper, tes bagian 8, versi runtime terkunci per situs, dan peluncuran
   bertahap.
5. **Mengaku "Apache".** Plugin bisa mengandalkan perilaku Apache yang tidak kita tiru.
   Diredam dengan `<IfModule>` dan korpus uji per plugin.
6. **Perawatan versi PHP.** Turun menjadi pembaruan manifest, kecuali di macOS.
7. **Pasar bergerak.** EnvKit dan FlyEnv mengisi celah "gratis dan lengkap". Kaitan ke
   hosting dan kompatibilitas `.htaccess` adalah pembeda yang tidak mereka miliki.

## 13. Positioning

- "Ditulis di Rust" tetap bukan manfaat bagi pengguna.
- Audiens terkecil yang layak: developer dan agensi WordPress/Laravel yang memakai, atau
  bisa diajak memakai, hosting milik owner. Bukan "semua pengguna XAMPP".
- Polanya sudah terbukti: perusahaan hosting merilis alat dev lokal sebagai pintu masuk
  (Local milik WP Engine, DevKinsta milik Kinsta, Studio milik WordPress.com, Herd dengan
  Forge). Contoh-contoh ini dari pengetahuan umum, tidak diverifikasi ulang di sesi ini.
- Janji produk: "Taruh folder, situs jalan. Yang jalan di laptop Anda, jalan dengan aturan
  yang sama di hosting, termasuk `.htaccess`-nya."
- Untuk pelanggan pindahan dari cPanel: tidak perlu menulis ulang aturan.
- Gratis dan terbuka. Pendapatan dari hosting.

## 14. Yang belum diverifikasi

- Dukungan Xdebug dan SAPI cgi di StaticPHP (static-php.dev tidak bisa diakses).
- Kosakata `.htaccess` di luar sampel 7 plugin; WooCommerce, Wordfence, dan plugin
  keamanan belum diperiksa.
- Perilaku `*.localhost` di Safari dan di alat CLI tiap OS; port per situs sebagai cadangan.
- Mengikat port 80 tanpa hak admin di macOS dan Linux.
- Cara termurah untuk `.test` di Windows. Kandidat: aturan NRPT ke DNS lokal (hickory)
  dengan satu kali izin admin, atau penyuntingan file hosts lewat helper.
- Mesin regex Rust yang paling dekat dengan PCRE untuk mod_rewrite.
- Build Redis atau Valkey untuk Windows yang sehat dan lisensinya cocok.
- Apache di Windows: build berasal dari pihak ketiga, dan cara terbaik menyambungkannya
  ke pool php-cgi (mod_fcgid atau mod_proxy_fcgi) belum diuji.
- Pengukuran bagian 5 sudah diulang dengan MariaDB (bagian 19), tetapi hanya untuk
  susunan yang dipilih dan nginx sebagai patokan, di mesin yang sama dengan `wrk`.
- Konkurensi diukur dengan beban jenuh; uji dengan laju tetap di bawah titik jenuh belum
  dilakukan. HTTP/2, HTTP/3, dan nginx lewat HTTPS belum diukur.
- Cara membatasi antrean PHP per situs di Caddy (dan jawaban 503 cepat) belum diuji.
- Cache halaman tingkat platform untuk Caddy tidak lagi direncanakan (bagian 19). Yang
  belum: aturan berkas cache untuk plugin selain WP Super Cache, dan cookie keranjang
  belanja WooCommerce.
- SSL diuji terhadap CA ACME lokal, belum terhadap Let's Encrypt dengan domain sungguhan.
- Batas laju Let's Encrypt saat ini (jumlah sertifikat per domain terdaftar, kegagalan
  validasi per jam) belum dicek ulang di sesi ini.
- CA cadangan dan DNS-01 untuk wildcard di mod_md belum diuji.
- Modul ACME bawaan nginx (v0.4.1, Mei 2026) ada tetapi tidak diuji.
- Apakah pemasangan CA lokal Caddy di Windows bisa tanpa hak admin.
- Octane dengan FrankenPHP di Windows.
- Apa yang dipakai hosting milik owner sekarang sebagai web server di container. Bila sudah Apache,
  itu mengubah hitungan untung-rugi lapisan web.
- Model harga Herd Pro (tahunan atau sekali bayar).

## 15. Prinsip kerja (tetap dari v1)

- KISS di atas DRY. Aturan tiga: abstraksi baru dibuat setelah duplikasi muncul tiga kali.
- Make it work, make it right, make it fast.
- Review teknis gaya Linus: blak-blakan, fokus substansi, sebut desain yang buruk.
- UI/UX gaya Steve Jobs: sederhana, fokus, berani berkata "tidak" pada fitur.

## 16. Hasil spike S1 dan langkah berikutnya

### Yang dibuat

Satu file Rust (`src/main.rs`, sekitar 590 baris termasuk tes) dengan tiga dependency:
`ctrlc`, `libc`, `serde_json`. Binary `jamu` berukuran 0,8 MB.

- `jamu serve [DIR]`: mengenali project, menulis konfigurasi Caddy dan php-fpm ke folder
  state di luar project, menyalakan keduanya, dan mengawasinya.
- `jamu serve [DIR] --domain NAMA`: mode production di port 80 dan 443 dengan sertifikat
  otomatis.
- `jamu detect [DIR]`: menampilkan hasil deteksi.

### Yang terbukti (Linux, PHP 8.3.6, Caddy 2.11.9)

| Uji | Hasil |
|---|---|
| Laravel 13 baru tanpa file config | Halaman depan 200, `/up` 200, query SQLite, sesi, 404 dari Laravel |
| WordPress 7.2-beta | Instalasi lewat browser, halaman depan, permalink cantik, `/wp-admin/`, REST API, file statis, gzip |
| Upload 100 MB | 0,7 detik, hash cocok. 200 MB ditolak 413 seketika karena batas 128 MB |
| Respons dialirkan | Byte pertama 0,003 detik dari respons 2 detik |
| Dotfile | `/.env`, `/.git/config`, basis data SQLite `.ht.*` semuanya 404 |
| PHP di folder upload WordPress | 403, termasuk `evil.php/x` dan `EVIL.PhP` |
| Dev hanya di loopback | Tidak bisa dijangkau dari alamat IP mesin |
| Port bentrok | "port 8080 sedang dipakai oleh caddy (pid N); memakai 8081" |
| php-fpm atau Caddy dimatikan paksa | Dinyalakan ulang dalam 1 detik, situs kembali 200 |
| Supervisor dimatikan paksa | Caddy dan php-fpm ikut mati |
| Berhenti dengan SIGTERM | Tidak ada proses tersisa |
| Mode domain dengan CA ACME uji | HTTPS sah dalam 0,6 detik, HTTP/2, HTTP/3 diiklankan, HTTP dialihkan 308 |
| Nyala ulang di mode domain | Sertifikat yang sama dipakai lagi, tidak diterbitkan ulang |
| Memori diam | Supervisor 1,1 MB, Caddy 46 MB, master php-fpm 32 MB |

### Yang ditemukan dan diperbaiki selama spike

1. Larangan PHP di folder upload bisa dilewati dengan `evil.php/x`, karena Caddy memecah
   path pada `.php`. Pola larangannya diperluas.
2. Caddy menampung respons PHP sampai selesai. Diperbaiki dengan `flush_interval -1`.
3. Setelah master php-fpm mati mendadak, pekerjanya yang tertinggal masih memegang soket
   sehingga master baru gagal menyala. Sekarang seluruh grup proses dibersihkan dulu.
4. Supervisor panik bila stderr ditutup. Penulisan log tidak lagi boleh menjatuhkannya.

### Yang belum, dan keterbatasan spike

- Sertifikat dari staging Let's Encrypt pada domain sungguhan belum diuji; sandbox ini
  tidak bisa dijangkau dari internet.
- Diuji sebagai root, sehingga php-fpm berjalan sebagai root. Jalur pengguna biasa belum diuji.
- Versi PHP dari `composer.json` baru ditampilkan, belum dipakai untuk memilih versi.
- Belum ada `jamu.toml`, worker, penjadwal, mail, MariaDB, dashboard.
- Linux saja (memakai `prctl` dan `/proc`).
- Caddy diambil dari image Docker dan php-fpm dari apt; manifest komponen belum ada.
- Di dev, fatal error PHP dijawab dengan status 200 berisi pesan error (bawaan PHP saat
  `display_errors` aktif).
- Batas body 128 MB dan batas memori PHP masih ditulis tetap di kode.

### Langkah berikutnya

1. S1 sisanya: jalankan `jamu serve --domain` di VPS dengan domain sungguhan terhadap
   staging Let's Encrypt, sebagai pengguna non-root.
2. S3: sudah dikerjakan, lihat bagian 17. Sisanya adalah deploy di Dokploy sungguhan.
3. S2: sudah dikerjakan, lihat bagian 18.
4. S4: sudah dikerjakan, lihat bagian 19.

Kode spike boleh dibuang. Yang dibawa ke fase 1 adalah aturan deteksi, isi konfigurasi
Caddy dan php-fpm, serta empat temuan di atas sebagai tes.

## 17. Hasil spike S3: image container

### Yang dibuat

- `Dockerfile`: image resmi `php:8.4-fpm`, ditambah Caddy dan `jamu`. Tidak ada yang
  berjalan sebagai root. `jamu` adalah PID 1.
- `jamu serve --behind-proxy`: HTTP di semua antarmuka, port 8080, untuk container di
  belakang Traefik. Ini mode ketiga, di samping dev (loopback) dan `--domain` (memegang 443).
- `jamu health`: dipakai `HEALTHCHECK`. Lulus hanya bila Caddy menerima koneksi dan php-fpm
  menjawab ping. Path ping hanya dijawab untuk permintaan dari loopback.
- `tests/image.sh`: uji asap image. `.github/workflows/ci.yml`: tes supervisor, build
  image, dan uji asap di setiap push.

Pemakaian untuk Laravel atau PHP biasa cukup dua baris:

```dockerfile
FROM jamuphp:8.4
COPY --chown=www-data:www-data . /app
```

### Di belakang proxy tanpa menyentuh config situs

HTTPS berakhir di Traefik, jadi PHP melihat HTTP biasa. Tanpa penanganan, WordPress
berputar-putar dalam redirect dan Laravel membuat URL `http://`. Jalan keluarnya ada di
konfigurasi Caddy yang dibangkitkan:

- Bila `X-Forwarded-Proto: https` datang dari alamat jaringan privat, PHP menerima
  `HTTPS=on`. Dari alamat lain header itu diabaikan.
- `REMOTE_ADDR` diisi alamat pengunjung, diambil dari `X-Forwarded-For` hanya bila
  pengirimnya proxy di jaringan privat.

Hasilnya `wp-config.php` dan `TrustProxies` Laravel tidak perlu disentuh.

### Yang terbukti (Docker 29.9, Traefik v3, PHP 8.4.27, MariaDB 11)

Lingkungan uji meniru Dokploy: Traefik v3 dengan HTTPS di jaringan `dokploy-network`,
routing lewat label container ke port 8080.

| Uji | Hasil |
|---|---|
| Laravel 13 lewat Traefik | HTTP/2, `/up` 200, `url()` menghasilkan `https://`, `request()->secure()` benar, sesi, SQLite |
| WordPress 7.2-beta + MariaDB lewat Traefik | Instalasi, halaman depan (semua URL `https://`), login (cookie `Secure`), dashboard tanpa redirect berulang, permalink cantik, REST API |
| Permintaan ke port HTTP Traefik | WordPress mengalihkan ke `https://` |
| Upload 100 MB lewat Traefik | 0,35 detik, hash cocok. 200 MB ditolak 413 |
| Respons dialirkan lewat Traefik | Byte pertama 0,007 detik dari respons 2 detik |
| `.env`, PHP di folder upload | 404 dan 403 |
| Path ping dari luar | Tidak dijawab php-fpm; jatuh ke situs (404) |
| Master php-fpm atau Caddy di-`kill -9` | Kembali 200 dalam 1,7 detik, tidak ada zombie |
| Proses latar yatim dari kode situs | Dipungut `jamu` setelah selesai, tidak ada zombie |
| `docker stop` | 0,24 detik, kode keluar 0 |
| `docker stop` saat ada permintaan 3 detik | Permintaan selesai 200, lalu container berhenti |
| `docker stop` saat ada permintaan 30 detik | Diputus setelah 8 detik, kode keluar 0, tetap di bawah batas 10 detik Docker |
| Container baru | Sehat dan dirutekan Traefik 2,6 detik setelah `docker start` |
| Caddy dibekukan | Container `unhealthy` setelah 36 detik, Traefik berhenti merutekan (404); pulih sendiri setelah dicairkan |
| Filesystem read-only, atau UID sembarang | Jalan; konfigurasi pindah ke `/tmp/jamuphp-<uid>` |
| Read-only tanpa `/tmp` yang bisa ditulis | Berhenti dengan pesan yang menyebut jalan keluarnya |
| Volume WordPress milik root | Peringatan saat start: `wp-content` tidak bisa ditulis oleh uid 33 |
| Memori diam (`docker stats`) | Laravel 21 MiB, WordPress 50 MiB setelah dipakai |

### Yang ditemukan dan diperbaiki

1. Image resmi PHP memasang `STOPSIGNAL SIGQUIT`. `jamu` sebagai PID 1 tidak menanganinya,
   sehingga `docker stop` menunggu 10 detik lalu membunuh paksa (kode 137). Dockerfile
   sekarang memasang `STOPSIGNAL SIGTERM`.
2. Saat berhenti, Caddy dan php-fpm dihentikan bersamaan, jadi permintaan yang sedang
   berjalan dijawab 502. Sekarang berurutan: Caddy menghabiskan permintaan dulu, baru
   php-fpm dihentikan.
3. Sebagai PID 1, `jamu` mewarisi proses yatim. Sekarang ia memungutnya.
4. Caddy mencoba menulis ke `HOME`, yang di container tidak bisa ditulis. Folder data dan
   config Caddy sekarang diarahkan ke folder state.
5. php-fpm tidak mengenali ping karena Caddy mengirim `SCRIPT_NAME` kosong untuk path
   tanpa `.php`. Nilainya sekarang diisi terang-terangan.

### Extension PHP

Tiga lapis, urut dari yang paling sering dipakai:

1. Satu set tetap yang semuanya aktif, gabungan kebutuhan WordPress dan Laravel. Di image:
   `bcmath exif gd igbinary imagick intl mysqli pdo_mysql pdo_pgsql redis zip`, di atas
   bawaan image resmi (curl, dom, fileinfo, mbstring, openssl, pdo_sqlite, sodium, opcache).
   Dipasang dengan `install-php-extensions`.
2. Pemeriksaan sebelum start: `ext-*` di `composer.json` dan `composer.lock` dibandingkan
   dengan modul yang termuat. Belum dibuat; usulnya masuk fase 1.
3. Pintu keluar: `ARG PHP_EXTENSIONS` saat build image, atau PHP sendiri lewat `JAMU_PHP_FPM`.

Di Windows sumbernya binary resmi NTS, dan extension tambahan lewat PIE, yang mengunduh DLL
jadi. Build PHP sendiri (StaticPHP) ditunda; build statis penuh tidak bisa memuat extension
tambahan. Xdebug dikirim tapi mati. `pcntl` dan `posix` tidak ada di Windows, jadi Laravel
Horizon tidak jalan di sana.

### Yang belum, dan keterbatasan

- Build penuh dengan semua extension hanya diuji di CI (GitHub Actions, lulus bersama uji
  asap yang memeriksa setiap extension). Sandbox pengujian memblokir repo Debian dan PECL,
  jadi uji Traefik, WordPress, dan jalur negatif di atas memakai image dengan Dockerfile
  yang sama kecuali satu baris: `docker-php-ext-install bcmath exif mysqli pdo_mysql`.
  Akibatnya `gd`, `imagick`, `intl`, `redis`, dan `zip` belum diuji dengan situs sungguhan.
- Belum di-deploy ke Dokploy sungguhan. Yang diuji adalah tiruannya (Traefik v3 + label).
- Image diterbitkan CI ke GHCR setelah lulus uji asap: `main` menjadi
  `ghcr.io/cacacuil/jamuphp:8.4`, branch `spike/x` menjadi `:spike-x`. Keduanya bisa
  ditarik tanpa login. Hanya amd64; arm64 belum.
- Ukuran image 775 MB, 704 MB di antaranya image dasar `php:8.4-fpm` yang membawa compiler.
- WordPress diuji dengan file inti di volume, dari luar image. Bila nanti situs WordPress
  di-deploy, usulnya image terpisah (`FROM jamuphp` + file inti) dengan `wp-content`
  sebagai volume. Belum dibuat.
- `HEALTHCHECK` tiap 10 detik membuat satu pekerja php-fpm tetap hidup, jadi "ondemand"
  tidak pernah turun ke nol pekerja di container.
- Perlu 36 detik sampai container dinyatakan `unhealthy`. Bisa dipercepat dengan interval
  lebih pendek.
- `X-Forwarded-Proto` dipercaya dari semua alamat privat, termasuk container lain di
  jaringan Docker yang sama.
- Belum ada batas CPU atau memori per situs; itu diatur di Dokploy.

### Langkah berikutnya

1. Deploy satu situs di Dokploy dari image GHCR. Port container: 8080.
2. S2 sudah dikerjakan (bagian 18) dan S4 juga (bagian 19).

## 18. Hasil spike S2: Windows

### Yang dibuat

- `jamu` yang sama, satu kode, berjalan di Windows. Semua yang berbeda antar-OS ada di satu
  modul (`os`) dengan dua isi: Unix dan Windows. Sisanya (deteksi, Caddyfile, pengawasan,
  urutan berhenti) tidak bercabang.
- PHP dijalankan `php-cgi.exe` dari zip NTS resmi, sebagai server FastCGI di satu port
  loopback. Caddy tetap lapisan web.
- `php.ini` dibangkitkan ke folder state dan ditunjuk dengan `-c`, karena `php-cgi` tidak
  mengenal `php_admin_value`. Zip resmi datang tanpa `php.ini`, jadi tidak satu pun
  extension aktif; `jamu` menyalakan extension yang filenya ada di folder `ext`.
- Binary dicari di samping `jamu.exe` lebih dulu, baru `PATH`. Satu folder berisi
  `jamu.exe`, `caddy.exe`, dan `php\` langsung jalan tanpa pemasangan dan tanpa variabel.
- `tests/e2e.sh`: 46 cek ujung ke ujung (PHP biasa, Laravel, WordPress), skrip yang sama
  untuk Linux dan Windows. `tests/windows.sh`: 55 cek khusus Windows.
  `tests/windows_stop.py`: berhenti dengan rapi.
- CI: job `windows` (PHP 8.4 dan 8.5) merakit `jamu` + Caddy + zip resmi PHP, menjalankan
  ketiga uji, lalu menyimpan rakitannya sebagai artifact `jamuphp-windows-php8.4` dan
  `-php8.5` (56 MB) supaya bisa dicoba di Windows sungguhan.

### Koreksi atas rencana: pool tidak perlu dibangun

Bagian 5 menulis bahwa `PHP_FCGI_CHILDREN` tidak berlaku di Windows, sehingga supervisor
harus menjalankan N proses di N port, membagi permintaan, dan mengganti proses yang keluar.
Itu salah. Kode `sapi/cgi/cgi_main.c` (cabang PHP-8.4) punya jalur Windows sendiri: proses
induk membuat pekerja dengan `CreateProcess`, mewariskan soket dengar ke semuanya,
memasukkan mereka ke Job Object dengan `KILL_ON_JOB_CLOSE`, dan mengganti pekerja yang
keluar.

Akibatnya rancangan menjadi lebih sederhana daripada rencana:

- Satu upstream di Caddy, bukan daftar alamat dengan pembagi beban.
- Permintaan yang melebihi jumlah pekerja antre di soket, tidak ditolak.
- Saat pekerja didaur ulang, soket tetap dipegang induk. Tidak ada jeda "port tertutup".
- Supervisor tetap mengawasi dua proses, sama seperti di Linux.

Kode pool yang sudah dirancang tidak jadi ditulis. Ini "rakit, jangan bangun" yang nyaris
terlewat: sumbernya dibaca dulu sebelum menulis.

### Hasil

Diuji di GitHub Actions `windows-latest` (4 core), Caddy 2.11.9, PHP 8.4.27 dan 8.5.12.
Angka kedua versi PHP nyaris sama; yang ditulis di sini PHP 8.4.

| Yang diuji | Hasil |
|---|---|
| Uji ujung ke ujung yang sama dengan Linux: PHP biasa, Laravel 13, WordPress + SQLite | 45 lulus; satu cek (hanya loopback) dipindah ke uji khusus Windows |
| Folder project bernama `Situs Saya é` (spasi dan huruf non-ASCII) | Jalan |
| Unggah 100 MB, isi sama; 200 MB | 200; 413 |
| Streaming | Byte pertama di bawah 1 detik |
| Mulai sampai siap | 0,8 sampai 1,0 detik |
| Extension | 22 menyala, termasuk curl, gd, intl, mbstring, exif, mysqli, pdo_mysql, pdo_pgsql, pdo_sqlite, sodium, zip |
| opcache | Aktif dan dipakai bersama semua pekerja (6032 hit terbaca dari satu pekerja) |
| 6000 permintaan, 16 sekaligus | Semua 200 dalam 3,7 detik. 14 PID pekerja berbeda: daur ulang tiap 500 permintaan tidak menjatuhkan satu permintaan pun |
| 24 permintaan 0,7 detik sekaligus, 4 pekerja | Semua 200 dalam 5,0 detik: antre, tidak ditolak |
| Induk `php-cgi` dimatikan; Caddy dimatikan | 200 lagi dalam 5 detik |
| `jamu` dimatikan paksa (`taskkill /F`) | `caddy.exe` dan `php-cgi.exe` ikut mati |
| Ctrl-Break saat permintaan 3 detik sedang berjalan | Permintaan selesai dengan 200, `jamu` keluar kode 0 dalam 2,7 detik, tidak ada proses tertinggal |
| Alamat lain milik mesin itu (bukan loopback) | Tidak bisa terhubung |
| Port sudah dipakai | Keluar dengan pesan yang menyebut portnya |
| Perubahan file PHP | Langsung terlihat di permintaan berikutnya |
| Memori diam | `php-cgi` 5 proses 138 MB, Caddy 33 MB, `jamu` 4 MB |
| 30 URL yang mencoba membaca sumber PHP atau dotfile | Tidak ada yang bocor (lihat di bawah) |

### Sumber PHP dan dotfile: 30 URL, tidak ada yang bocor

Bagian 8 mencatat keanehan nama file Windows sebagai tes wajib. Semuanya dicoba terhadap
situs yang file PHP-nya memuat penanda rahasia:

- `/INDEX.PHP`, `/index.pHp`, `/index.php.`, `/index.php..`, `/index.php%20`: dijalankan
  PHP, bukan disajikan sebagai file.
- `/index.php::$DATA` dan variasinya: Caddy tidak menemukannya sebagai file, permintaan
  jatuh ke `index.php` seperti URL biasa.
- Nama pendek ala DOS. Volume itu memang membuatnya: `.env` punya nama kedua `ENV~1`,
  `.git` menjadi `GIT~1`. Caddy menjawab 400 untuk `/ENV~1`, `/GIT~1/config`, dan
  `/KONFIG~1.PHP`.
- `/.env`, `/.ENV`, `/.env.`, `/.env%20`, `/.env::$DATA`, `/.git/config`, `/.GIT/config`: 404.
- PHP di folder unggahan WordPress, termasuk `X.PHP`, `x.php.`, `x.php%20`, `x.php/y`,
  dan `/WP-CONTENT/UPLOADS/x.php`: 403, tidak dijalankan.

Perlindungan ini hampir seluruhnya milik Caddy, bukan kode kita. Satu yang kita tambah:
aturan folder unggahan kini juga menolak `.php` yang diikuti titik atau spasi, karena
Windows menganggap `x.php.` sama dengan `x.php`. Itu ditutup sebelum diuji, jadi tidak
terbukti bahwa tanpa itu bisa dieksploitasi.

### Temuan

1. **HTTPS dari PHP gagal di zip resmi.** `curl` menjawab "unable to get local issuer
   certificate" karena zip itu tidak membawa daftar CA. `file_get_contents` berhasil,
   sebab stream OpenSSL di Windows memakai penyimpanan sertifikat sistem. Dengan
   `cacert.pem` di `php\extras\ssl\`, `jamu` mengisi `curl.cainfo` dan `openssl.cafile`,
   dan keduanya berhasil. Distribusi Windows harus membawa `cacert.pem`; tanpa itu klien
   HTTP Laravel (Guzzle) gagal pada permintaan HTTPS pertama.
2. **Memori adalah harga Windows.** `php-cgi` tidak punya mode ondemand: semua pekerja
   hidup terus, sekitar 27 MB per proses. Delapan pekerja diam memakai 247 MB, jadi
   jumlahnya dijadikan 4 sampai 8 mengikuti jumlah core (138 MB di mesin 4 core). Di
   Linux dengan php-fpm ondemand angkanya 21 MB.
3. **opcache menahan perubahan kode sampai 2 detik.** Di mode dev sekarang diperiksa di
   setiap permintaan, di kedua OS.
4. **PHP 8.5 tidak punya `php_opcache.dll`** karena opcache sudah menyatu. Aturan "nyalakan
   bila filenya ada" bekerja untuk 8.4 dan 8.5 tanpa cabang per versi.
5. **Spasi di dalam regex memotong token Caddyfile.** `[. ]` membuat aturan folder
   unggahan cocok dengan semua URL, dan seluruh situs WordPress menjawab 403. Ketahuan
   oleh uji ujung ke ujung dalam hitungan menit. Spasi ditulis `\x20`.

### Yang belum, dan keterbatasan

- Hanya diuji di runner CI (Windows Server), belum di Windows 10 atau 11. Antivirus,
  SmartScreen, dan dialog firewall belum pernah dilihat. Mode dev hanya mengikat loopback
  dan tidak lagi menyentuh `0.0.0.0` saat memeriksa port, jadi dialog firewall seharusnya
  tidak muncul; itu dugaan sampai ada yang melihatnya.
- Ctrl-C di jendela terminal sungguhan dan menutup jendelanya belum diuji. Yang diuji
  adalah Ctrl-Break dari proses lain, yang memakai penangan yang sama.
- Nama pengguna Windows dengan spasi atau huruf non-ASCII (jalur negatif A5) belum diuji:
  folder state ada di `%LOCALAPPDATA%\jamuphp`. Yang diuji baru path project. Path lebih
  dari 260 karakter juga belum.
- Port `php-cgi` dipilih dengan mengikat port 0 lalu melepasnya. Ada celah sempit proses
  lain merebut port itu sebelum `php-cgi` mengikatnya; akibatnya `php-cgi` gagal start dan
  diulang dengan port yang sama.
- `jamu health` hanya untuk php-fpm. `php-cgi` tidak punya ping.
- Pekerja `php-cgi` tidak punya stderr. Pesan galat PHP lewat FastCGI ke log Caddy;
  tampilannya belum diperiksa.
- `--domain` dan `--behind-proxy` tidak diuji di Windows. Windows hanya target dev.
- Extension tambahan lewat PIE, Xdebug, dan ionCube belum disentuh.
- `cacert.pem` di rakitan CI diunduh dari curl.se saat CI berjalan. Cara membawanya di
  distribusi (dan memperbaruinya) belum diputuskan.
- Rakitan belum bertanda tangan dan belum berupa installer. Windows arm64 belum.
- `tests/e2e.sh` di Linux baru dijalankan di sandbox (lulus 46 untuk php-fpm, dan 46 untuk
  `php-cgi` lewat `JAMU_PHP_CGI`). Di CI, Linux baru diuji lewat tes unit dan uji asap image.

### Langkah berikutnya

1. Owner mencoba rakitan di Windows 10 atau 11: unduh artifact `jamuphp-windows-php8.4`
   dari run CI terakhir, ekstrak, lalu `jamu.exe serve C:\path\ke\project`. Yang perlu
   dilihat: dialog firewall, antivirus, dan Ctrl-C.
2. Sisa S3 (deploy di Dokploy) dan sisa S1 (domain sungguhan).
3. S4 sudah dikerjakan (bagian 19).

## 19. Hasil spike S4: MariaDB dan cache halaman

### Yang dibuat

Semuanya di `docs/benchmark/`, tidak ada perubahan pada `jamu`:

- `s4-setup.sh` menyiapkan MariaDB 11 dan dua salinan WordPress yang isinya sama, satu
  memakai MariaDB dan satu SQLite. `s4-seed.php` mengisinya dengan 20 tulisan.
- `s4-cache.php` menyalakan dan mematikan WP Super Cache tanpa membuka wp-admin.
- `s4.sh` menjalankan semua beban, tiap beban tiga kali 15 detik setelah pemanasan 5 detik.
- `s4-supercache.caddy` adalah kandidat aturan Caddy untuk menyajikan berkas cache.
  `s4-nginx.conf` adalah nginx pembanding.
- `s4-results.txt` berisi satu baris per jalan; `s4-ringkas.sh` menghitung mediannya.

### Cara ukur

- Yang diukur adalah image `jamuphp:8.4` apa adanya: Caddy 2.11.9, PHP 8.4.27 dengan
  `php.ini-production` (OPcache aktif dan memeriksa perubahan berkas), php-fpm ondemand
  dengan 4 pekerja, yaitu angka yang dihitung `jamu` dari jatah 2 core.
- MariaDB 11.8.9 dengan konfigurasi bawaan image, di 2 core yang sama dengan server web,
  seperti VPS kecil yang menjalankan keduanya.
- WordPress 7.2-beta, tema bawaan, 21 tulisan. Beranda 126 KB. WP Super Cache 3.1.5 dengan
  setelan yang disarankan plugin: mode "Simple" dan tanpa cache untuk pengunjung yang
  dikenal. Mode "Expert" tidak dipakai karena bergantung pada `.htaccess`.
- `wrk` 4.1.0 di 2 core lain, 8 koneksi kecuali disebut lain, langsung ke IP container.
- Tiga pintu masuk ke php-fpm yang sama di container `jamu`, supaya selisihnya hanya
  berasal dari lapisan web: **J** `jamu` apa adanya; **C** Caddyfile buatan `jamu`
  ditambah aturan cache; **N** nginx 1.31.8.
- Sebelum tiap beban, satu permintaan diperiksa: kode jawaban, ukuran, kompresi, dan ada
  tidaknya penanda cache. Tidak ada jawaban selain 200 dan tidak ada galat soket di semua
  jalan.

Yang sengaja tidak diulang: Apache dan FrankenPHP. Owner sudah memutuskan Caddy, jadi
keduanya tidak lagi mengubah keputusan apa pun. nginx tinggal sebagai patokan. Angka di
sini juga tidak bisa dijajarkan dengan tabel bagian 5: versi PHP, ukuran halaman, dan
jalur jaringannya berbeda. Karena itu SQLite diukur ulang dengan cara yang sama.

### Hasil

Median tiga jalan, req/dtk; dalam kurung nilai terendah dan tertinggi.

| Beban | J: `jamu` apa adanya | C: `jamu` + aturan cache | N: nginx |
|---|---|---|---|
| Halaman dinamis, MariaDB | 25,6 (25,1-25,7) | - | 24,6 (24,6-24,9) |
| Halaman dinamis, SQLite | 22,2 (21,9-22,4) | - | - |
| Halaman dinamis, cache menyala, pengunjung ber-cookie | 25,7 (25,2-26,0) | - | - |
| Halaman dinamis, 64 koneksi | 23,9 (21,1-25,0) | - | - |
| Halaman cache 126 KB, disajikan PHP | 2.160 (2.077-2.180) | - | 2.963 (2.887-2.995) |
| Halaman cache 126 KB, disajikan server web | - | 9.714 (9.597-9.749) | 52.881 (51.581-54.008) |
| Halaman cache, header kompresi peramban | 1.260 (1.257-1.277) | 2.702 (2.610-2.703) | 1.958 (1.455-1.960) |
| Halaman cache, 64 koneksi | 2.116 (2.041-2.253) | 10.314 (10.169-10.615) | - |
| Halaman cache saat 300 koneksi menunggu PHP | **0,5** (0,5-0,5) | 4.846 (4.549-5.130) | 23.570 (23.135-25.035) |
| File statis 7 KB | 17.185 (16.538-17.870) | - | 80.500 (80.277-82.252) |
| File statis 7 KB, header kompresi peramban | 11.907 (11.877-12.237) | - | 19.307 (19.014-19.350) |

Waktu tunggu (median p50): halaman dinamis 308 ms; 64 koneksi ke halaman dinamis
2,5 detik; halaman cache lewat PHP 3,4 ms; lewat Caddy 0,7 ms; halaman cache
lewat PHP saat PHP kebanjiran 11,5 detik.

Memori (PSS):

| Keadaan | PSS | Rincian |
|---|---|---|
| Container `jamu`, baru hidup | 54 MB | Caddy 27 MB, php-fpm 25 MB (induk dan 1 pekerja), `jamu` 2 MB |
| Container `jamu`, sesudah beban dinamis | 112 MB | Caddy 32 MB, php-fpm 78 MB (induk dan 4 pekerja), `jamu` 1 MB |
| Container `jamu`, 20 detik diam | 89 MB | Caddy 32 MB, php-fpm 56 MB (induk dan 2 pekerja), `jamu` 1 MB |
| MariaDB, konfigurasi bawaan, diam | 125 MB | di luar container `jamu` |

### Yang ditunjukkan angka ini

1. **MariaDB hanya sedikit lebih cepat dari SQLite**: 25,6 lawan 22,2 req/dtk, selisih
   15% (11% di jalan pertama). Dugaan di bagian 5 bahwa angka dinamis rendah karena
   SQLite ternyata salah. Leher botolnya PHP itu sendiri:
   sekitar 80 milidetik CPU per halaman di mesin ini, jadi 2 core memberi 25 halaman per
   detik, apa pun databasenya.
2. **Lapisan web tetap tidak memengaruhi halaman dinamis.** Caddy dan nginx di depan
   php-fpm yang sama memberi angka yang sama.
3. **Plugin cache langsung bekerja di image tanpa `.htaccess` dan tanpa pekerjaan apa pun
   di `jamu`.** Halaman cache yang disajikan PHP 84 kali lebih cepat daripada halaman
   dinamis. Untuk situs yang menjadi sasaran produk ini, angka itu sudah lebih dari cukup.
4. **Menyajikan berkas cache dari Caddy memberi 4,5 kali lagi**, tetapi itu bukan
   alasan utamanya. Alasan utamanya nomor 5.
5. **Saat PHP kebanjiran, halaman cache yang disajikan PHP ikut mati.** Dengan 300 koneksi
   menunggu halaman yang tidak bisa di-cache, pengunjung halaman cache menunggu 11,5
   detik. Di dunia nyata banjir semacam ini datang dari tebak sandi di `wp-login.php`,
   `xmlrpc.php`, dan bot yang menambahkan query acak. Dengan aturan cache di Caddy,
   halaman cache tetap dijawab dalam 1,2 milidetik pada banjir yang sama. Membatasi
   antrean PHP (jalur negatif H1) tidak menyelamatkan halaman cache: ia hanya membuat
   kegagalannya cepat, karena halaman cache tetap berebut pekerja yang sama.
6. **Untuk halaman cache yang dimampatkan, selisih Caddy dan nginx berbalik.** Peramban
   selalu meminta kompresi, dan dua-duanya memampatkan 126 KB di setiap permintaan. Caddy
   memilih zstd dan mengalahkan nginx dengan gzip. Angka tanpa kompresi, tempat nginx
   lima kali lebih cepat, tidak pernah dialami pengunjung sungguhan. Ongkos terbesar
   halaman cache sekarang adalah pemampatan berulang, bukan server web.
7. **Memori satu situs 54 MB saat baru hidup dan 89 MB dua puluh detik setelah beban.**
   Caddy dengan konfigurasi buatan `jamu` memakai 27-44 MB, lebih rendah dari 45-57 MB
   di bagian 5. Memori setelah diam lama tidak diukur. MariaDB dengan
   konfigurasi bawaan memakai 125 MB sebelum melayani apa pun, lebih besar dari seluruh
   lapisan web. Untuk hosting banyak situs, cara berbagi MariaDB lebih menentukan memori
   daripada pilihan server web.
8. **Pemeriksaan kesehatan container menahan satu pekerja PHP tetap hidup.** `jamu health`
   berjalan tiap 10 detik dan batas diam pekerja ondemand juga 10 detik, jadi selalu ada
   minimal satu pekerja. Terlihat pada container diam: pekerja yang sama masih hidup
   setelah 47 detik. Itu belasan MB per situs yang tidak pernah dilepas; imbalannya
   permintaan pertama tidak perlu menunggu pekerja dibuat.
9. **64 koneksi ke halaman dinamis tidak merobohkan apa pun**: tetap 24 req/dtk, tanpa
   galat, tetapi tiap pengunjung menunggu 2,5 detik. Ini jalur negatif H1 yang
   terukur ulang dengan MariaDB.

### Aturan cache kandidat dan jalur negatifnya

Aturannya ada di `docs/benchmark/s4-supercache.caddy`: bila permintaannya GET atau HEAD,
tanpa query, tanpa cookie pengguna yang dikenal, dan berkas cache untuk host dan path itu
ada, Caddy menyajikan berkasnya. Selain itu permintaan diteruskan ke PHP seperti biasa.
Tidak ada cache yang dibangun: plugin tetap yang membuat dan menghapus berkasnya.

| Permintaan lewat pintu C | Dijawab oleh |
|---|---|
| `GET /` dan `HEAD /` | Caddy, isi sama persis dengan `index.html` plugin |
| `GET /` dengan `X-Forwarded-Proto: https` | Caddy, isi sama persis dengan `index-https.html` |
| `GET /tulisan-uji-7/` yang sudah di-cache | Caddy |
| `GET /tulisan-uji-11/` yang belum di-cache | PHP, lalu berkasnya terbentuk |
| `GET /?x=1` | PHP |
| `GET /` dengan cookie komentator atau `wordpress_logged_in_*` | PHP, halaman tanpa cache |
| `POST /` | PHP |
| Tulisan disunting | Plugin menandai berkasnya; permintaan berikutnya ke PHP dan memuat isi baru, lalu kembali ke Caddy |
| Folder plugin dihapus tanpa dinonaktifkan | **Caddy terus menyajikan halaman lama.** Lewat pintu J, WordPress menyajikan halaman segar |

Dua temuan dari uji ini:

1. **Versi pertama aturan salah untuk HTTPS.** Plugin menyimpan halaman HTTPS di
   `index-https.html`. Aturan yang hanya mengenal `index.html` tidak pernah kena di situs
   HTTPS, yaitu semua situs production, dan pada situs campuran ia menyajikan varian HTTP
   ke pengunjung HTTPS. Aturan sekarang ada dua, satu per skema. Pengukuran diulang penuh
   dengan aturan yang sudah dibetulkan; angka di atas dari pengulangan itu, dan jalan
   pertama memberi angka yang sama dalam selisih beberapa persen.
2. **Halaman basi bila plugin dihapus paksa** (jalur negatif H9). Ini harga dari
   melewati PHP: tidak ada lagi yang memeriksa apakah plugin masih ada. Sebelum aturan
   ini masuk ke `jamu`, ia harus bersyarat pada keberadaan berkas plugin.

### Rekomendasi

Jalur negatif H7 semula berbunyi "cache halaman disediakan platform". Setelah diukur,
saran saya: **jangan bangun cache sendiri.** Bagian yang sulit dari cache adalah tahu
kapan halaman harus dibuang, dan plugin sudah melakukannya dengan benar dari dalam
WordPress. Yang perlu dirakit hanya dua hal:

1. **Hosting memasang plugin cache saat situs dibuat.** Ini urusan templat situs di
   hosting, bukan urusan `jamu`. Tanpa `jamu` berbuat apa-apa, hasilnya sudah lebih dari 80 kali
   lebih cepat.
2. **`jamu` menulis aturan berkas cache untuk project WordPress** di fase 2 (production),
   bukan fase 1. Alasannya nomor 5 di atas, bukan kecepatan. Syarat sebelum masuk: aturan
   bersyarat pada berkas plugin (H9), bentuk `protocol https` untuk mode `--domain`, dan
   tabel jalur negatif di atas menjadi tes.

Yang saya sarankan ditolak: modul cache untuk Caddy (butuh build Caddy sendiri, dan
aturan pengecualian per plugin harus ditulis ulang) dan dukungan banyak plugin cache
sekaligus. Satu plugin yang didukung dengan benar lebih baik daripada lima yang setengah.

Keputusan ini milik owner: bila ketergantungan pada tata letak berkas satu plugin
dianggap terlalu mahal, pilihan yang jujur adalah tidak menulis aturan sama sekali dan
menerima bahwa banjir ke PHP ikut mematikan halaman cache.

### Yang belum, dan keterbatasan

- `wrk` dan server berjalan di mesin yang sama, dengan 4 core. Angka absolutnya milik
  mesin ini; yang bisa dipegang adalah perbandingannya.
- Beban jenuh, bukan laju tetap. Satu halaman (beranda) saja yang diukur.
- MariaDB memakai konfigurasi bawaan dan data yang sangat kecil (21 tulisan). Situs dengan
  ribuan tulisan atau WooCommerce bisa berperilaku lain.
- OPcache di image memeriksa perubahan berkas tiap 2 detik. Mematikannya di production
  tidak diukur.
- Aturan cache hanya untuk WP Super Cache. Cookie keranjang WooCommerce, varian seluler,
  dan berkas `.gz` buatan plugin belum disentuh. Menyajikan berkas yang sudah dimampatkan
  (`file_server` dengan `precompressed`) akan menghapus ongkos di nomor 6 dan belum dicoba.
- Plugin membuat satu folder cache untuk setiap nilai header `Host` yang datang. Di
  belakang proxy yang menyaring host hal itu tidak terjangkau; pada mode `--domain`
  belum diperiksa.
- Aturan bersyarat pada berkas plugin (H9) belum ditulis.
- Pintu C dijalankan dengan biner Caddy yang sama tetapi di container terpisah, bukan
  dibangkitkan oleh `jamu`. `jamu` sendiri tidak diubah di spike ini.
- Dari bagian 5 yang tidak diulang: Apache, FrankenPHP, HTTPS, HTTP/2 dan HTTP/3, ribuan
  koneksi diam.

### Langkah berikutnya

Keempat spike sudah dikerjakan. Yang masih terbuka sebelum fase 1:

1. Owner mencoba rakitan Windows di Windows 10 atau 11 (bagian 18).
2. Deploy satu situs di Dokploy dari image GHCR, dan uji `--domain` terhadap staging
   Let's Encrypt dengan domain sungguhan (sisa S3 dan S1).
3. Owner memutuskan soal aturan berkas cache di atas. Sudah: disetujui, masuk fase 2.

## 20. Fase 1: core headless (sudah di `main`)

Kode spike tidak dibuang; ia dirapikan menjadi dasar fase 1. Dikerjakan di branch `fase-1`
dan digabung ke `main` atas permintaan owner, sehingga image `:8.4` di GHCR sudah berisi
hasil fase ini.

### Yang dibuat

- **Kode dipecah menjadi modul**, tanpa mengubah perilaku: `detect` (mengenali project),
  `settings` (`jamu.toml`), `config` (Caddyfile dan konfigurasi PHP), `serve`, `supervise`,
  `cli` (`jamu php`, `jamu composer`), `paths`, dan dua berkas khusus OS.
- **Tes daftar wajib bagian 8** ada di `tests/e2e.sh`: satu skrip, dijalankan terhadap
  php-fpm di Linux dan `php-cgi` di Windows. Situs ujinya (Laravel baru dan WordPress
  dengan SQLite) dibuat `tests/sites.sh`. CI mendapat job `linux`, sehingga jalur
  developer Linux (paket distro, bukan image) ikut teruji di setiap push.
- **`jamu.toml`**, opsional. Lima kunci, tidak lebih:

  | Kunci | Arti | Yang ditolak |
  |---|---|---|
  | `php = "8.4"` | Janji: project ini hanya dijalankan PHP 8.4.x. `serve` dan `jamu php` menolak versi lain dengan satu kalimat. | Selain dua angka, misalnya `"8"` atau `"^8.2"`. |
  | `root = "web"` | Document root, menimpa hasil deteksi. | Yang bukan folder, atau yang keluar dari folder project. |
  | `scheduler = true` | Menyalakan atau mematikan penjadwal Laravel. | Selain `true` dan `false`. |
  | `[ini]` | Pengaturan php.ini untuk project ini. Berlaku untuk web, pekerja, penjadwal, dan `jamu php`. | Nama di luar huruf, angka, titik, garis bawah; nilai lebih dari satu baris. |
  | `[[worker]]` dengan `name` dan `run` | Proses latar yang ikut dinyalakan. | Nama ganda, nama yang dipakai `jamu` sendiri (`caddy`, `php-fpm`, `schedule`, ...), `run` kosong. |

  Kunci yang tidak dikenal ditolak, supaya salah ketik (`rot = "public"`) tidak diam-diam
  diabaikan. Semua penolakan terjadi sebelum ada proses yang dinyalakan.
- **Pekerja.** `run` dijalankan lewat shell (`sh -c`, di Windows `cmd /C`) di folder
  project. `php` di dalamnya adalah PHP yang melayani situs, dengan `[ini]` yang sama.
  Pekerja yang mati dinyalakan ulang dengan jeda yang bertambah (1 sampai 30 detik), sama
  seperti Caddy dan PHP. Pekerja tidak menahan situs: situs dianggap siap begitu Caddy dan
  PHP hidup, dan pekerja yang gagal dinyalakan tidak menjatuhkan situsnya. Saat berhenti,
  pekerja diberi tahu paling awal dan ditunggu paling akhir (9 detik), supaya
  `queue:work` sempat menyelesaikan tugas yang sedang dikerjakan.
- **Penjadwal Laravel.** `php artisan schedule:run` dijalankan tiap pergantian menit,
  seperti cron. Menyala dengan sendirinya di `--domain` dan `--behind-proxy`; mati di mode
  dev, kecuali `scheduler = true`. Hanya untuk Laravel. wp-cron menunggu fase 2.
- **`jamu php` dan `jamu composer`.** Dijalankan dari folder project. Keduanya memakai PHP
  yang sama dengan `serve`, dengan `[ini]` dan pemeriksaan versi yang sama, dan meneruskan
  kode keluar. Di Windows ini yang membuat `php` punya ekstensi dan daftar CA tanpa
  php.ini yang ditulis tangan. Composer dicari di samping `jamu` (`composer.phar`), lalu
  di `PATH`. Composer ikut di image dan di rakitan Windows.
- **Catatan galat PHP sampai ke log `jamu`** di kedua jalur, dengan awalan `[php-fpm]`
  atau `[php-cgi]`.

### Temuan dari tes bagian 8

Enam hal yang salah dan tidak akan terlihat tanpa tes itu:

1. **`X-Forwarded-For` bisa dipalsukan.** Di mode `--behind-proxy`, Caddy mengambil alamat
   paling kiri dari header itu, yaitu alamat yang ditulis pengunjung sendiri. Pembatas
   laju dan log di aplikasi jadi bisa dikelabui. Diperbaiki dengan
   `trusted_proxies_strict`: alamat diambil dari kanan, dari proxy yang dipercaya.
2. **Laravel menjalankan berkas `.php` apa pun di `public/`.** Berkas yang ditaruh
   penyusup lewat celah unggah langsung bisa dieksekusi. Sekarang hanya `index.php` yang
   dijalankan; permintaan ke berkas `.php` lain diteruskan ke Laravel dan berakhir 404.
3. **Jenis isi berkas statis bergantung OS.** Caddy menanyakannya ke sistem operasi.
   Windows menjawab `text/plain` untuk `.mjs`, sehingga browser menolak modul JavaScript;
   di image container tabelnya kosong. `jamu` kini menetapkan sendiri jenis isi 31 akhiran
   yang umum, sama di semua OS. Jawaban dari PHP tidak disentuh.
4. **`error_log()` di jalur `php-cgi` tidak terlihat.** Catatannya dikirim lewat FastCGI
   dan dibuang Caddy. Sekarang ditangkap dan ditampilkan.
5. **`jamu.toml` ikut tersaji** bila document root adalah folder project itu sendiri
   (WordPress, PHP biasa). Sekarang selalu 404, termasuk varian huruf besar dan titik atau
   spasi di ujung nama.
6. **Header `Proxy` (httpoxy)** ternyata sudah dibuang Caddy. Tidak ada yang perlu
   diperbaiki; tesnya tetap ada supaya tidak diam-diam berubah.

### Hal yang perlu diketahui sebelum memakainya

- **Penjadwal dan lebih dari satu replika.** Bila satu situs dijalankan di dua container,
  keduanya menjalankan `schedule:run`. Matikan di semua kecuali satu
  (`scheduler = false`), atau pakai `onOneServer()` milik Laravel. Ini keputusan bawaan
  yang bisa dibalik: menyala di production berarti situs Laravel yang baru di-deploy
  langsung punya penjadwal tanpa cron, dengan harga kejutan ini pada pemasangan
  berreplika.
- **Tugas terjadwal tidak saling menunggu.** Sama seperti cron: bila yang sebelumnya
  belum selesai, yang berikutnya tetap dijalankan. Pencegahnya `withoutOverlapping()`.
- **Batas untuk web tidak berlaku di baris perintah.** `memory_limit` 256M dan batas
  unggahan adalah bawaan untuk web. Pekerja, penjadwal, dan Composer hanya menerima
  `[ini]`, karena kebutuhan mereka lain (Composer sering butuh memori lebih).
- **Perubahan `jamu.toml` baru berlaku setelah `serve` dinyalakan ulang.**

### Hasil uji

`tests/e2e.sh` berisi 148 pemeriksaan. Di CI semuanya lulus di Linux (php-fpm 8.3 dari paket
distro) dan di Windows dengan PHP 8.4 dan 8.5 (147 pemeriksaan; satu yang dilewati diuji
`tests/windows.sh` dengan cara Windows). Uji khusus Windows dan uji asap image juga lulus.
Situs Laravel yang diuji dibuat CI lewat `jamu composer create-project` lalu
`jamu php artisan migrate`, tanpa file config dan tanpa php.ini yang ditulis tangan. Itu
syarat lulus fase 1 yang pertama.

Kinerja di Windows tidak berubah dibanding S2: siap dalam 0,8 detik, 6000 permintaan
dalam 3,7 detik.

### Yang belum

- Log akses. Saat ini hanya galat yang tercatat.
- Pemilihan versi PHP (lebih dari satu versi terpasang, unduh otomatis): fase 3.
- `redis` di rakitan Windows: fase 3.
- wp-cron, aturan multisite, aturan berkas cache: fase 2.
- Uji `--domain` dengan domain sungguhan dan deploy di Dokploy dari image GHCR masih sisa
  S1 dan S3, dan tetap perlu dilakukan owner.

## 21. Fase 2: production (sedang berjalan)

Dikerjakan di branch `fase-2` dan digabung ke `main` atas permintaan owner, sehingga image
`:8.4` di GHCR sudah berisi hasil fase ini, termasuk `msmtp`. Yang masih berjalan adalah
uji dengan layanan sungguhan dan masa 30 hari di syarat lulus.

### Yang dibuat

- **Aturan berkas cache WP Super Cache** (rekomendasi bagian 19). Halaman yang sudah dibuat
  plugin disajikan Caddy tanpa menyentuh PHP. Syaratnya sama dengan aturan `.htaccess`
  buatan plugin: hanya GET dan HEAD, tanpa query string, alamat berakhir garis miring,
  dan pengunjung tidak membawa cookie login, komentar, atau tulisan bersandi. Halaman HTTP
  dan HTTPS memakai berkas terpisah, mengikuti plugin; di belakang proxy, HTTPS dikenali
  dari `X-Forwarded-Proto` yang datang dari jaringan privat. Aturan hanya berlaku selama
  berkas plugin masih ada, diperiksa di setiap permintaan: bila folder plugin dihapus tanpa
  dinonaktifkan, berkas cache yang tertinggal tidak disajikan selamanya.
- **Multisite subdirektori.** Dikenali dari `MULTISITE` dan `SUBDOMAIN_INSTALL` di
  `wp-config.php`; aturan tulis ulangnya (`/toko/wp-admin/...` ke `/wp-admin/...`) dipasang
  sebelum aturan tolak. Multisite subdomain tidak butuh aturan tambahan.
- **wp-cron.** Penjadwal yang sama dengan Laravel menjalankan `php wp-cron.php` tiap
  menit, dan tidak memulai yang baru selama yang lama belum selesai. Menyala dengan
  sendirinya di `--domain` dan `--behind-proxy`, mati di mode dev.
- **Log akses.** Satu baris per permintaan:
  `[akses] <alamat pengunjung> <METODE> <uri> <kode> <lama> ms`. Pemeriksaan kesehatan
  (`jamu health`) tidak dicatat, supaya log tidak berisi satu baris tiap 10 detik.
- **`jamu check`.** Opsi yang sama dengan `serve`. Menulis konfigurasi ke subfolder sendiri
  lalu meminta Caddy dan php-fpm mengujinya, jadi situs yang sedang jalan tidak terganggu.
  Yang diperiksa: `jamu.toml`, versi PHP, kebutuhan paket Composer
  (`composer check-platform-reqs`), folder yang harus bisa ditulis, `APP_KEY` dan
  `APP_DEBUG` Laravel (variabel lingkungan menang atas `.env`; `APP_DEBUG` hanya masalah di
  mode production), rekaman DNS tiap `--domain`, `.htaccess` yang menutup folder, dan relay
  SMTP. Keluar dengan kode 1 bila ada masalah, jadi bisa menggagalkan deploy (F1).
- **Beberapa domain.** `--domain` boleh diulang; semua nama dilayani situs yang sama dan
  mendapat sertifikat. Nama domain, `--email`, dan `--acme-ca` diperiksa sebelum ditulis ke
  konfigurasi Caddy, karena di hosting nilainya bisa berasal dari isian pelanggan.
- **Kabar sertifikat.** Terbit, diperpanjang, gagal, dan kapan dicoba lagi ditulis sebagai
  baris `jamu: sertifikat ...`. Bila CA menolak, alasannya diterjemahkan (DNS belum benar,
  server tidak terjangkau, rekaman CAA, batas laju) dan kalimat asli dari CA ikut
  ditampilkan.
- **Mode teruskan email.** Bila `JAMU_SMTP_HOST` diisi, `sendmail_path` PHP diarahkan ke
  `jamu sendmail`, yang menyerahkan email ke `msmtp` untuk diantar ke relay. Berlaku untuk
  web, pekerja, penjadwal, dan `jamu php`. Tidak ada server email, antrean, atau proses
  latar yang ditambahkan. Variabelnya enam, tercantum di README.

### Temuan

1. **G1 sudah dijaga Caddy.** Dibaca dari kode certmagic 0.25.6: setelah penerbitan gagal,
   percobaan berikutnya dilakukan ke endpoint staging CA dulu, dan baru kembali ke
   endpoint production setelah staging berhasil (berlaku untuk CA bawaan, Let's Encrypt).
   Domain yang DNS-nya belum benar tidak menghabiskan jatah di CA production. Karena itu pemeriksaan DNS buatan sendiri sebelum
   meminta sertifikat tidak dibangun; yang dibangun adalah menampilkan penjelasan dari CA.
2. **Alamat server sendiri tidak bisa diketahui dari dalam** (NAT, container). `check`
   menampilkan alamat yang ditunjuk DNS untuk dicocokkan orang, dan hanya nama tanpa
   rekaman sama sekali yang dihitung masalah. Rekaman AAAA lama (G6) ikut terlihat di
   daftar itu.
3. **`mail()` dan parameter kelima.** PHP menempelkan parameter kelima `mail()` ke perintah
   sendmail. Itu jalur serangan lama (opsi `-X` menulis berkas, `-C` membaca konfigurasi).
   `jamu sendmail` hanya mengambil `-f` dengan alamat yang sah dan membuang sisanya.
4. **msmtp menolak masuk dengan sandi tanpa TLS.** Karena itu `JAMU_SMTP_TLS=off` bersama
   `JAMU_SMTP_USER` ditolak saat `serve` dinyalakan, bukan saat email pertama gagal.
5. **Email tanpa pengirim ditolak msmtp.** `mail($ke, $judul, $isi)` polos tidak membawa
   header From. Untuk itu ada `JAMU_MAIL_FROM`; tanpa keduanya `mail()` mengembalikan
   `false` dan sebabnya ditulis di log.
6. **Runner Windows di CI sesekali hilang di tengah uji** (dua dari sepuluh job): tanpa
   log, dan batas waktu job tidak berlaku. Sebab yang paling masuk akal: `kill` di Git
   Bash untuk program Windows asli mematikan juga semua "keturunan" yang ia temukan lewat
   PID induk. PID di Windows dipakai ulang, dan di runner ada proses sistem yang induknya
   sudah mati (`winlogon.exe`, `wininit.exe`, `csrss.exe`, `explorer.exe`). Bila `jamu`
   kebetulan mendapat PID bekas induk itu, `kill` ikut mematikan proses sistem dan
   mesinnya jatuh. Ini belum dibuktikan langsung. Yang sudah dilakukan: tes kini
   mematikan `jamu` lewat PID Windows-nya dengan `taskkill`, dan anak-anaknya ikut mati
   lewat Job Object. Sejak itu dua job Windows lulus; butuh lebih banyak putaran untuk
   yakin. Ini soal skrip uji, bukan soal `jamu`.

### Hal yang perlu diketahui sebelum memakainya

- **Folder yang ditutup `.htaccess` tetap terbuka.** `check` melaporkannya sebagai masalah,
  tetapi `serve` tidak menolaknya. Lihat keputusan nomor 1 di bawah.
- **wp-cron dari baris perintah hanya mengurus situs utama multisite.** Situs lain di
  jaringan tetap mengandalkan wp-cron bawaan WordPress yang dipicu pengunjung.
- **Perubahan menjadi multisite baru dikenali setelah `jamu` dinyalakan ulang.**
- **Mode "mobile device support" WP Super Cache tidak didukung.** Bawaannya mati.
- **Batas laju email menghitung email, bukan penerima,** dalam jendela satu jam menurut jam dinding. Email
  pertama yang ditolak dicatat; berikutnya ditolak tanpa baris log sampai jam berganti.
- **Batas laju bukan pagar terhadap kode yang sudah dikuasai penyerang.** Sandi relay ada
  di lingkungan proses PHP, sama seperti sandi database. Kode yang bisa menjalankan
  perintah bisa berbicara SMTP sendiri. Yang ditahan adalah penyalahgunaan `mail()` lewat
  formulir atau plugin yang bocor. Pagar yang sesungguhnya adalah akun relay per situs
  dengan batasnya sendiri di sisi penyedia.
- **Mode teruskan email belum ada di Windows.** Penangkap email untuk development adalah
  pekerjaan fase 3.

### Hasil uji

`tests/e2e.sh` kini berisi 271 pemeriksaan di jalur php-fpm dan 269 di jalur
`php-cgi` (keduanya dijalankan di Linux; di CI jalur kedua berjalan di Windows). Tes unit
34. Mode teruskan email diuji terhadap server SMTP palsu (`tests/smtp_palsu.py`): amplop,
isi, `-f`, parameter liar, relay mati, pengirim bawaan, dan batas laju.

Yang hanya diuji dari kode sumber dan tes unit, belum terhadap layanan sungguhan:

- Baris "sertifikat terbit", "diperpanjang", dan "pemeriksaan domain gagal". Baris gagal
  dan "dicoba lagi" sudah diuji dengan CA yang tidak terjangkau.
- Masuk ke relay dengan akun dan TLS. Jalur sandi (`--passwordeval`) diuji tangan terhadap
  server palsu dengan CRAM-MD5; TLS ke penyedia sungguhan belum.

### Keputusan yang menunggu owner

1. **Folder yang ditutup `.htaccess`.** Rekomendasi: `serve` menolak sendiri folder yang
   `.htaccess`-nya berisi larangan untuk seluruh folder (`Deny from all`,
   `Require all denied`), dibaca saat dinyalakan. Itu kasus yang tidak ambigu dan paling
   berbahaya bila terlewat (C1, F1). Ini menyentuh keputusan menunda `.htaccess`, jadi
   tidak saya kerjakan tanpa persetujuan. Kelemahannya: berkas yang dibuat plugin sesudah
   `serve` jalan baru berlaku setelah dinyalakan ulang.
2. **wp-cron menyala sendiri di production.** Sama seperti penjadwal Laravel, dengan
   kejutan yang sama pada pemasangan berreplika. Bisa dimatikan dengan `scheduler = false`.
3. **Nama variabel dan bawaan mode email:** `JAMU_SMTP_*`, `JAMU_MAIL_FROM`,
   `JAMU_MAIL_LIMIT` dengan bawaan 200 email per jam. Angka itu perkiraan, bukan hasil ukur.
4. **msmtp, bukan kode SMTP sendiri.** Mengikuti "rakit, jangan bangun". Harganya satu
   paket tambahan di image.
5. **Log JSON ditunda.** Belum ada yang membacanya; formatnya lebih baik ditentukan saat
   ada pemakai pertama (misalnya pengumpul log di hosting).

### Yang belum

- Sisa hari sertifikat (G3) dan peringatan sebelum kedaluwarsa. Caddy memperpanjang
  sendiri dan kegagalannya kini tercatat; tampilan sisa hari menunggu dashboard.
- Pemeriksaan huruf besar-kecil nama berkas (C8) di `check`.
- Log JSON.
- Uji `--domain` dengan domain sungguhan terhadap staging Let's Encrypt, uji relay SMTP
  sungguhan, dan deploy di Dokploy dari image GHCR: perlu dilakukan owner.
- Syarat lulus fase ini (sejumlah situs berjalan 30 hari) baru bisa dimulai setelah itu.

## 22. Kestabilan dan performa (sudah di `main`)

Dikerjakan di branch `stabil-performa`. Sasaran dari owner: performa dan kestabilan.
Pengerasan keamanan ditunda. Aturannya tetap: ukur dulu, ubah hanya yang terukur.

### Yang diukur

Sandbox 4 core, PHP 8.3.6 dari paket distro, Caddy 2.11.9, `wrk`. Angka req/dtk hanya
untuk dibandingkan satu sama lain, bukan dengan bagian 19.

| Yang diuji | Hasil |
|---|---|
| Halaman PHP yang tidur 45 detik (`max_execution_time` 30) | Dijawab 200 setelah 45 detik. Waktu menunggu tidak dihitung PHP: tidak ada batas (jalur negatif D3) |
| Koneksi yang tidak menyelesaikan header | Ditutup Caddy setelah 60 detik |
| 10 POST yang menjanjikan body lalu diam | Tidak satu pun pekerja PHP menyala dan situs tetap menjawab. Koneksinya masih terbuka saat ditutup di detik ke-43 |
| Memori pekerja Laravel di bawah beban, 8 pekerja | PSS total 53 MB; RSS puncak satu pekerja 31-47 MB |
| opcache, Laravel halaman sambutan | 553 skrip, 1.058 dari 16.229 kunci, 23,5 dari 128 MB, string 5,0 dari 8,0 MB |
| `opcache.interned_strings_buffer` 4, 8, 32 MB | 537, 533, 514 req/dtk: tidak ada selisih |
| JIT (`tracing`, 64 MB), dua putaran | 597 dan 578 lawan 543 dan 576 req/dtk tanpa JIT: di dalam derau |
| `pm.max_requests = 500` pada halaman paling ringan, dua putaran tiga jalan | 5.293-5.947 req/dtk dengan, 5.523-6.216 tanpa; tidak ada permintaan gagal dari 59.879 |
| opcache di image `php:8.4-fpm` | Aktif; kini diperiksa uji image di CI |

### Yang dibuat

1. **Batas waktu satu permintaan** (D3). Kunci baru `timeout` di `jamu.toml`, bawaan 300
   detik, 0 berarti tanpa batas. Tepat di batasnya Caddy menjawab 504 (`read_timeout`);
   dua detik kemudian php-fpm menghentikan pekerjanya (`request_terminate_timeout`) dan
   log menyebut permintaannya. Tanpa ini delapan permintaan yang menggantung ke API atau
   database yang mati membekukan seluruh situs tanpa batas waktu.
2. **Jumlah pekerja mengikuti memori container** (F3). Sebelumnya dua kali jumlah core,
   minimal 4, apa pun memorinya: di node 32 core tanpa kuota CPU itu 64 pekerja per situs.
   Sekarang juga dibatasi memori cgroup dibagi 64 MB. Container 512 MB mendapat 8 pekerja,
   256 MB mendapat 4. Tanpa batas memori, perilakunya tidak berubah.
3. **Batas unggahan satu sumber.** Caddy dulu dipatok 128 MB, sehingga `post_max_size`
   yang dinaikkan di `[ini]` tetap berakhir 413. Sekarang batas Caddy dibaca dari
   `post_max_size` yang berlaku.
4. **Pekerja php-fpm diganti tiap 500 permintaan**, sama seperti jalur `php-cgi`, supaya
   kebocoran memori plugin tidak menumpuk di situs yang tidak pernah sepi.
5. **Tiga kabar php-fpm ditulis ulang**: permintaan yang dihentikan karena batas waktu,
   semua pekerja sibuk, dan pekerja yang dimatikan sistem (biasanya memori container
   habis). Baris rutin pergantian pekerja dibuang dari log.

### Yang ditolak

- **Penyetelan opcache dan JIT.** Tidak ada selisih yang terukur; bawaan PHP dibiarkan.
- **Batas waktu body dan batas jumlah koneksi di Caddy** (H5, H6). Body yang lambat
  terbukti tidak menyandera pekerja PHP. Batas waktu body akan memutus unggahan besar
  dari sambungan lambat, dan Caddy bawaan tidak punya batas jumlah koneksi. Di belakang
  proxy, batas milik proxy yang berlaku.

### Hal yang perlu diketahui

- **Batas waktu dihitung sejak permintaan diteruskan ke PHP sampai jawabannya selesai.**
  Jawaban yang dialirkan ikut terpotong: aliran 6 detik dengan `timeout = 2` berhenti di
  detik ke-2 dengan kode 200 yang sudah terkirim. Situs dengan SSE atau unduhan panjang
  buatan PHP perlu menaikkan `timeout`.
- **Di jalur `php-cgi` (Windows) jawabannya 504 tetapi pekerjanya tidak dihentikan.**
  `php-cgi` tidak punya padanan `request_terminate_timeout`.
- **Angka 64 MB per pekerja adalah taksiran dari pengukuran situs kecil.** Situs dengan
  plugin berat bisa melewatinya; saat itu terjadi kernel mematikan satu pekerja dan
  log menyebut penyebabnya.
- **WordPress tidak diukur ulang di sini** karena situs uji lokal belum terpasang. Angka
  WordPress tetap dari bagian 19.

### Hasil uji

`tests/e2e.sh`: 278 pemeriksaan di jalur php-fpm dan 275 di jalur `php-cgi`, semuanya
lulus di sandbox. 39 tes unit. Uji image menambah dua pemeriksaan: opcache aktif, dan
jumlah pekerja mengikuti `--memory`.

### Yang belum

- **"jamu: siap" diumumkan walau php-fpm gagal menyala.** Terlihat saat pengukuran: siap
  diumumkan begitu port menerima koneksi. `jamu health` tetap gagal, jadi container tidak
  dirutekan, tetapi baris log itu menyesatkan.
- **Metrik.** Jumlah pekerja aktif dan panjang antrean belum bisa dilihat dari luar; yang
  ada hanya baris log "semua pekerja PHP sedang sibuk".
- **Uji beban lama.** Semua pengukuran di sini 8-15 detik. Kebocoran dan penumpukan baru
  terlihat dalam jam atau hari.
- **Tag image yang tetap per rilis.** Sekarang hanya `:8.4` yang terus ditimpa, jadi tidak
  ada versi untuk kembali bila rilis baru bermasalah.

## 23. Pembanding: FrankenPHP dan nginx + php-fpm (sudah di `main`)

Permintaan owner: uji dan bandingkan dengan yang lain, misalnya FrankenPHP. Skripnya
`docs/benchmark/banding/banding.sh`, dijalankan workflow `banding` (hanya bila diminta
atau di branch `banding`). Tabel lengkap: `docs/benchmark/banding/hasil-2026-11-14.md`.

### Cara mengukur

- Satu mesin CI (4 core, 15 GB), satu run, semua server bergantian di port yang sama
  dengan jaringan host. `wrk` 4 thread, 8 detik, tiga putaran, yang dicatat mediannya.
  `wrk` berjalan di mesin yang sama.
- PHP 8.4.27 di semuanya, opcache aktif (diperiksa skrip), JIT mati, `php.ini-production`,
  log akses menyala. Jumlah pekerja sama: 8 proses php-fpm, 8 thread FrankenPHP.
- **jamu**: image `jamuphp:8.4` apa adanya (Caddy 2.11.9 + php-fpm ondemand).
- **FrankenPHP klasik**: image resmi `dunglas/frankenphp:php8.4` (v1.13.5, Caddy 2.11.9)
  dengan opcache dipasang seperti anjuran dokumentasinya.
- **FrankenPHP worker**: image yang sama, Laravel lewat berkas worker milik Octane.
- **nginx + php-fpm**: nginx 1.26.3 ditambahkan ke image jamuphp, jadi PHP dan
  extension-nya persis sama; `pm = static`, pengaturan tercepat untuk php-fpm.
- Aplikasi: halaman PHP satu baris, berkas statis 7 KB, Laravel 13.42 (production,
  `artisan optimize`, sesi di cookie), WordPress 7.1.4 dengan SQLite dan tema bawaan.

### Hasil (req/dtk, 64 koneksi)

| Beban | jamu | FrankenPHP klasik | nginx + php-fpm | FrankenPHP worker |
|---|---|---|---|---|
| Halaman PHP satu baris | 10.360 | 19.661 | 27.517 | - |
| Berkas statis 7 KB | 23.577 | 26.274 | 99.821 | - |
| Laravel `/up` | 1.423 | 1.480 | 1.613 | 3.856 |
| Laravel `/` (halaman sambutan) | 1.103 | 1.135 | 1.232 | 2.263 |
| WordPress halaman depan | 63,5 | 66,6 | 66,5 | - |

Tidak ada satu pun permintaan gagal di seluruh tabel.

| Hal lain | jamu | FrankenPHP klasik | nginx + php-fpm | FrankenPHP worker |
|---|---|---|---|---|
| p99 Laravel `/up` | 54 ms | 53 ms | 47 ms | 29 ms |
| p99 WordPress | 1.690 ms | 1.020 ms | 1.590 ms | - |
| Memori Laravel di bawah beban | 72-76 MB | 62-64 MB | 56-60 MB | 74-75 MB |
| Memori Laravel saat diam | 45 MB | 43 MB | 51 MB | 91 MB |
| Memori WordPress di bawah beban | 148 MB | 133 MB | 129 MB | - |
| Siap melayani (Laravel) | 273 ms | 292 ms | 280 ms | 356 ms |
| Ukuran image dengan Laravel | 753 MB | 622 MB | 762 MB | 622 MB |

Image jamuphp lebih besar karena berisi 11 extension tambahan, Composer, dan msmtp;
image FrankenPHP di sini hanya ditambah opcache.

**Satu proses PHP crash selagi situs dibebani 16 koneksi** (`posix_kill` SIGSEGV ke
dirinya sendiri, meniru extension yang crash; container `--restart always`):

| | jamu | FrankenPHP klasik | nginx + php-fpm |
|---|---|---|---|
| Permintaan yang crash | 502 | koneksi putus | 502 |
| Permintaan lain yang gagal | 0 dari 109.709 | semua yang sedang berjalan | 0 dari 313.399 |
| Situs tidak menjawab | 0 ms | sekitar 300 ms | 0 ms |
| Container diulang Docker | tidak | ya, satu kali | tidak |

Jumlah galat FrankenPHP yang dicetak `wrk` (75.664) tidak dipakai: selama server mati
`wrk` mencoba menyambung berulang-ulang dan setiap percobaan dihitung.

### Yang terbaca dari angka itu

1. **Di aplikasi nyata jamu setara FrankenPHP klasik**: 3-4% di belakang pada Laravel,
   sama pada WordPress. Terhadap nginx + php-fpm jamu tertinggal 10-12% pada Laravel dan
   sama pada WordPress. Pada WordPress waktu habis di PHP, bukan di server.
2. **Di halaman yang hampir kosong jamu paling lambat**: separuh FrankenPHP, sepertiga
   nginx. Sebabnya jalur FastCGI di Caddy, bukan jamu: di sandbox, Caddy dengan Caddyfile
   lima baris ke php-fpm yang sama mencapai 7.900 req/dtk, nginx 20.600. Selisih itu
   sekitar 0,25 ms CPU per permintaan, dan itulah yang terlihat sebagai 10% di Laravel.
3. **Berkas statis: nginx empat kali Caddy.** jamu dan FrankenPHP sama-sama Caddy dan
   kini berselisih 10%.
4. **Worker mode adalah satu-satunya loncatan besar**: Laravel 2 sampai 2,7 kali lebih
   cepat dan p99 separuhnya, dengan memori diam dua kali lipat. Ini benchmark aplikasi
   nyata yang disyaratkan baris "Worker mode" di bagian 5.
5. **Kestabilan: php-fpm mengurung crash di satu proses.** Di FrankenPHP satu crash PHP
   menjatuhkan seluruh server; situs pulih setelah Docker menyalakannya ulang.
6. Waktu siap dan memori diam praktis sama di ketiganya.

### Yang diperbaiki di jamu karena pengukuran ini

Di run pertama berkas statis jamu 19% di bawah FrankenPHP walau keduanya Caddy 2.11.9.
Sebabnya dicari di sandbox dengan Caddy yang sama:

| Berkas statis 7 KB, sandbox | req/dtk |
|---|---|
| jamu sebelum perbaikan | 12.700-14.300 |
| Caddy saja, Caddyfile buatan jamu | 14.300-15.900 |
| Caddy saja, tanpa aturan jenis berkas | 17.800-20.200 |
| jamu sesudah dua perbaikan | 16.000-16.600 |

1. **Jenis berkas dicari sekali per permintaan.** 31 pasang matcher dan `header` diganti
   satu `map` dengan kunci regex yang tidak peka huruf besar. Uji e2e baru: akhiran
   huruf besar (`BESAR.MJS`).
2. **Log akses: hanya lima bagian yang dipakai yang diuraikan.** Sebelumnya seluruh JSON
   baris itu, termasuk semua header, dibangun menjadi pohon; di 15.000 req/dtk itu 7%
   CPU mesin.

Di CI, jamu terhadap FrankenPHP pada berkas statis naik dari 81% ke 90%, pada halaman
PHP satu baris dari 49% ke 53%. Pada Laravel tidak ada perubahan yang berarti (95% ke
96-97%, di dalam derau).

### Yang diukur dan ditolak

- **Melonggarkan pengumpul sampah Caddy (`GOGC=400`).** Halaman satu baris +15%, berkas
  statis +24%, Laravel tidak berubah (535-575 req/dtk dengan, 534-540 tanpa), dan memori
  Caddy naik dari 60 ke 87 MB. Tidak ada untungnya di aplikasi nyata.

### Batas pengukuran ini

- Satu run di mesin CI bersama; selisih di bawah 5% jangan dibaca sebagai selisih.
  Antara run pertama dan terakhir angka mutlak berbeda sampai 10%.
- WordPress memakai SQLite, bukan MySQL. Basis datanya sama di ketiga server, tetapi
  angka mutlaknya bukan angka production.
- Putaran 8 detik: tidak ada yang bisa dikatakan tentang kebocoran atau jam-jam panjang.
- Belum diukur: HTTPS, HTTP/2, unggahan, banyak situs dalam satu node, container dengan
  batas memori kecil, dan WordPress dengan cache halaman.
- nginx di image Debian meneruskan `Host` tanpa port ke PHP; di konfigurasi pembanding
  itu dibetulkan supaya WordPress tidak mengalihkan setiap permintaan.

### Hasil uji

`tests/e2e.sh`: 279 pemeriksaan di jalur php-fpm dan 276 di jalur `php-cgi`, semuanya
lulus di sandbox. 39 tes unit.

## 24. Performa jalur PHP dan worker mode (sudah di `main`)

Permintaan owner: perbaiki performanya; kuncinya kecepatan dan kestabilan. Bagian 23
menunjukkan jamu paling lambat di halaman yang hampir kosong, jadi yang dicari di sini:
ke mana waktu Caddy habis di jalur PHP, dan apa yang benar-benar menaikkan angka di
aplikasi nyata.

### Cara mencari

Caddy dijalankan dengan Caddyfile buatan jamu ditambah `admin localhost:2019`, lalu
profil CPU diambil dari `/debug/pprof/profile` selagi `wrk` membebani. Angka req/dtk di
sandbox bergoyang sekitar 5%, jadi yang dibandingkan adalah CPU Caddy per permintaan
(dari `/proc/PID/stat`) pada putaran yang diselang-seling, dengan header seperti kiriman
browser.

Profil halaman PHP satu baris (23 detik CPU dalam 10 detik):

| Bagian | Porsi CPU Caddy |
|---|---|
| Panggilan sistem (baca, tulis, sambung, tutup) | 24% |
| Pengumpul sampah dan alokasi | 17-18% |
| Menyambung dan menutup soket ke php-fpm tiap permintaan | sekitar 11% |
| Menulis jawaban dengan flush di setiap tulisan | 9% |
| Menyiapkan catatan log per permintaan (`zap.Logger.With`) | 8-9% |
| Mencari berkas di disk (`try_files`) | 8-9% |
| Log akses | 6-8% |

Dua baris bisa dihapus tanpa mengubah perilaku; sisanya bawaan Caddy.

### Yang dibangun

1. **Keluaran PHP diteruskan paling lambat 20 ms, bukan di setiap tulisan.** Sebelumnya
   `flush_interval -1`. Di Caddy 2.11.9 klien FastCGI mengisi panjang jawaban dengan 0
   bila PHP tidak mengirim `Content-Length`, jadi tanpa `flush_interval` keluaran PHP
   ditahan sampai selesai; dengan `-1` pemampat dipaksa menutup blok di setiap potongan.
   20 ms tetap terasa langsung untuk halaman yang mencetak kemajuan, dan
   `text/event-stream` selalu diteruskan saat itu juga oleh Caddy. Uji e2e keluaran
   bertahap dan SSE tetap lulus.
2. **Pencatat transport FastCGI dikeluarkan dari log utama di jalur php-fpm.** Transport
   itu menyusun catatan berisi seluruh header dan variabel lingkungan untuk tiap
   permintaan walau tidak ada yang ditulis di tingkat WARN. Bila tidak ada log yang
   memuat pencatat itu, Caddy memberinya pencatat kosong dan ongkosnya hilang. Di jalur
   `php-cgi` pencatat itu tetap dipakai karena galat PHP lewat sana (`capture_stderr`).
   Yang hilang di jalur php-fpm hanya peringatan "jawaban lebih panjang dari
   `Content-Length`"; peringatan "jawaban terputus" berasal dari pencatat lain dan tetap ada.
3. **Laravel tanpa cache konfigurasi disebut oleh `jamu check` dan `jamu serve`** di mode
   production (bukan dev). Ini temuan terbesar di bagian ini, lihat tabel di bawah.

Hasil dua perubahan pertama, di sandbox lewat jamu sendiri (header browser, 32 koneksi):

| Beban | Sebelum | Sesudah |
|---|---|---|
| Halaman PHP satu baris | 5.895-6.320 req/dtk | 6.717-6.790 (+7 sampai 14%) |
| Halaman PHP 60 KB, dimampatkan | 1.695-1.803 req/dtk, p50 17-18 ms | 2.056-2.077 (+15 sampai 22%), p50 15 ms |
| Laravel `/up` | 947-1.009 | 992-1.044 (di dalam derau) |
| WordPress | 46,7-48,5 | 42,4-48,8 (tidak berubah) |

CPU Caddy per permintaan pada halaman satu baris turun dari 434 ke 367 mikrodetik
(-15%). Halaman 60 KB yang dimampatkan juga sedikit lebih kecil (11.229 ke 10.995 byte)
karena pemampat tidak lagi dipotong-potong.

Di CI (`docs/benchmark/banding/hasil-2026-11-14-b.md`, 64 koneksi): halaman satu baris
10.360 ke 11.541 req/dtk (+11%; terhadap FrankenPHP klasik dari 53% ke 58%). Berkas
statis, Laravel, dan WordPress tidak berubah (Laravel `/up` 1.423 ke 1.412, WordPress
63,5 ke 62,5). Kolom baru `64z` meminta jawaban dimampatkan seperti browser: pada
halaman sambutan Laravel jamu 975, FrankenPHP klasik 954, nginx 1.017 req/dtk.

**Yang terbaca:** penyetelan gateway terasa di halaman kecil dan halaman yang
dimampatkan, dan tidak terasa di Laravel dan WordPress, karena di sana waktunya habis di
PHP. Selisih jamu terhadap nginx di Laravel (sekitar 10%) adalah ongkos Caddy per
permintaan dan tidak bisa dihapus dari Caddyfile.

### Cache konfigurasi Laravel

Laravel yang sama, sandbox, `/up`, 32 koneksi:

| Keadaan | req/dtk |
|---|---|
| Apa adanya sesudah `composer install` | 532 |
| `APP_ENV=production`, `APP_DEBUG=false` | 556 |
| Rute dan event di-cache, konfigurasi tidak | 584 |
| Hanya cache konfigurasi | 978 |
| `php artisan optimize` | 979 |

Cache konfigurasi sendirian memberi +67%; tidak ada penyetelan server yang mendekati
itu. Catatan: `composer dump-autoload -o` menghapus cache itu (skrip
`post-autoload-dump` Laravel), dan peta kelasnya sendiri tidak menambah apa-apa.

Waktu bagian ini ditulis jamu hanya menyebut cache yang belum ada. Sejak revisi 17 jamu
membangunnya sendiri saat start (bagian 25). Di container variabel lingkungan baru ada
saat container menyala, jadi cache tidak boleh dibuat di Dockerfile; ia harus dibuat
saat start. Risikonya: aplikasi yang memanggil `env()` di luar berkas konfigurasi
mendapat `null` begitu konfigurasi di-cache.

### Yang diukur dan ditolak

- **Membuang header dari log akses dengan encoder `filter`.** Lebih lambat, bukan lebih
  cepat: CPU Caddy per permintaan +3% sampai +12%.
- **Mengeluarkan pencatat `reverse_proxy` juga.** Tidak menambah apa-apa di atas butir 2
  (-8,2% lawan -8,6%), dan peringatan "jawaban terputus" akan hilang.
- **Menjaga satu pekerja php-fpm tetap hidup, atau waktu diam yang lebih panjang.**
  Permintaan pertama sesudah diam 13 detik memakan 11-16 ms, yang hangat 4-8 ms: selisih
  6-11 ms sekali per masa diam, dibayar dengan memori satu pekerja sepanjang waktu. Di
  container, HEALTHCHECK 10 detik sudah menjaga satu pekerja hidup. Permintaan pertama
  sejak start (202 ms) adalah opcache yang masih kosong, bukan ondemand.
- **`opcache.validate_timestamps=0`.** 915-936 lawan 854-977 req/dtk: tidak terukur, dan
  deploy jadi butuh reload.

### Worker mode: tiga mesin, satu aplikasi

Bagian 23 menunjukkan worker mode FrankenPHP 2 sampai 2,7 kali lebih cepat tetapi satu
crash PHP menjatuhkan seluruh server. Pertanyaannya: apakah Laravel Octane dengan mesin
yang tiap pekerjanya proses sendiri (RoadRunner, Swoole) memberi kecepatan itu tanpa
kelemahan itu. Diukur di CI (`MODE=worker` di `banding.sh`, branch `banding-worker`;
tabel lengkap `docs/benchmark/banding/hasil-2026-11-14-worker.md`).

Susunan Octane: image aplikasi yang sama dengan jamu, ditambah `laravel/octane`,
extension `pcntl`, lalu `sockets` + program `rr` 2025.1.15 (RoadRunner) atau extension
`swoole` 6.2.5. `php artisan octane:start` dengan 8 pekerja di port loopback, dan Caddy
yang sama di depannya (`Caddyfile.octane`: berkas statis dari Caddy, sisanya
`reverse_proxy`). Opcache dinyalakan untuk PHP baris perintah dan pekerja diganti tiap
1.000 permintaan, sama dengan berkas worker FrankenPHP.

| Laravel, 64 koneksi | jamu (php-fpm) | FrankenPHP worker | Octane RoadRunner | Octane Swoole |
|---|---|---|---|---|
| `/up`, req/dtk | 1.411 | 3.465 | 1.548 | 2.274 |
| `/` (halaman sambutan), req/dtk | 1.013 | 2.050 | 939 | 1.077 |
| p99 `/up` | 58 ms | 32 ms | 119 ms | 390 ms |
| p99 `/up` di 8 koneksi | 12 ms | 5 ms | 370 ms | 175 ms |
| Memori di bawah beban | 74-79 MB | 73-77 MB | 427-437 MB | 332-347 MB |
| Memori saat diam | 45 MB | 106 MB | 304 MB | 299 MB |
| Siap melayani | 350 ms | 460 ms | 780 ms | 699 ms |
| Ukuran image | 764 MB | 633 MB | 824 MB | 836 MB |

Satu pekerja PHP crash (SIGSEGV) selagi `/up` dibebani 16 koneksi:

| | jamu (php-fpm) | FrankenPHP worker | Octane RoadRunner | Octane Swoole |
|---|---|---|---|---|
| Permintaan yang crash | 502 | koneksi putus | 500 | koneksi putus |
| Permintaan lain yang gagal | 0 dari 15.118 | semua yang sedang berjalan | 0 dari 17.292 | 0 dari 25.041 |
| Situs tidak menjawab | 0 ms | sekitar 800 ms | 0 ms | 0 ms |
| Container diulang Docker | tidak | ya | tidak | tidak |

Yang terbaca:

1. **Kestabilan: dugaan benar.** RoadRunner dan Swoole mengurung crash di satu pekerja,
   sama seperti php-fpm. Hanya FrankenPHP yang jatuh seluruhnya.
2. **Kecepatan: tidak seperti yang diharapkan.** RoadRunner hampir tidak lebih cepat dari
   php-fpm (+10% di `/up`, -7% di `/`). Swoole +61% di `/up` dan +6% di `/`. Keduanya jauh
   di bawah FrankenPHP worker.
3. **Ekor latensi Octane buruk**: p99 ratusan milidetik, bahkan di 8 koneksi. jamu 12 ms.
4. **Memori Octane empat sampai enam kali jamu**, juga saat diam, karena 8 pekerja hidup
   terus dan masing-masing memegang aplikasinya sendiri.

**Batas pengukuran ini, dibaca dulu sebelum memutuskan.** Angka Octane adalah susunan
bawaan `octane:start`, bukan hasil penyetelan. Run pertama tanpa opcache di baris perintah
dan dengan pekerja diganti tiap 500 permintaan memberi angka yang sama (RoadRunner `/up`
1.562, Swoole 2.250), jadi kedua hal itu bukan sebabnya. Sebab RoadRunner dan Swoole
tertinggal sejauh itu dari FrankenPHP worker, dan sebab ekor latensinya, **belum
ditemukan**; kemungkinan yang belum diperiksa: cara Swoole membagi koneksi dari Caddy ke
pekerja, dan ongkos menyalakan pekerja pengganti. Sampai itu jelas, angka kecepatan Octane
di sini adalah batas bawah, bukan putusan. Temuan kestabilan dan memorinya tidak
bergantung pada penyetelan itu.

Yang perlu ditambahkan ke image supaya Octane jalan (dicatat karena gagal dulu di CI):
`composer require` paket RoadRunner menuntut `ext-sockets`; Octane sendiri butuh `pcntl`
(tanpa itu: `Undefined constant SIGINT`).

### Keputusan yang menunggu owner

1. ~~Menggabungkan `stabil-performa` ke `main`.~~ Sudah, atas permintaan owner (15 Nov
   2026); image `:8.4` terbit ulang dari `main`.
2. ~~`artisan optimize` otomatis saat start di mode production.~~ Dikerjakan atas
   permintaan owner (18 Nov 2026), dengan sakelar `optimize` di `jamu.toml`: bagian 25.
3. **Worker mode.** Dari tiga mesin, tidak ada yang sekaligus cepat, hemat memori, dan
   tahan crash: FrankenPHP cepat dan hemat tetapi jatuh seluruhnya; RoadRunner dan Swoole
   tahan crash tetapi boros memori dan, sejauh yang terukur, tidak cukup cepat. Baris
   "Worker mode" di bagian 5 tetap: tunggu pelanggan Laravel yang meminta.

### Hasil uji

`tests/e2e.sh`: 290 pemeriksaan di jalur php-fpm dan 287 di jalur `php-cgi`, semuanya
lulus di sandbox. 40 tes unit. Yang baru: catatan cache konfigurasi Laravel di `check`
dan `serve` (muncul di production, diam di dev, hilang sesudah cache dibuat; diganti
bagian 25), dan keluaran
bertahap yang tetap mengalir saat dimampatkan dan untuk SSE. Dua tes terakhir dibuktikan
gagal dengan `flush_interval 5s`.

Satu kali di CI, `tests/image.sh` gagal di `jamu composer --version` lalu lulus di run
berikutnya dengan kode yang sama. Sebabnya belum diketahui; tes itu kini mencetak
keluaran perintahnya supaya kejadian berikutnya bisa dibaca.

## 25. `artisan optimize` otomatis (di branch `stabil-performa`, belum di `main`)

Permintaan owner: kerjakan `artisan optimize` otomatis dengan sakelar mati. Ini keputusan
nomor 2 di bagian 24; dasarnya pengukuran di sana (cache konfigurasi sendirian +67%).

### Yang dibuat

- **`src/laravel.rs`** (baru). Di mode production (`--domain` atau `--behind-proxy`),
  `jamu serve` menjalankan `php artisan optimize --no-ansi --no-interaction` untuk project
  Laravel yang `vendor`-nya sudah ada, **sebelum** php-fpm dan Caddy dinyalakan. Urutan
  itu disengaja: tidak ada permintaan yang membaca berkas cache yang sedang ditulis, dan
  di container variabel lingkungan baru ada saat itu.
- **Sakelar `optimize` di `jamu.toml`**, tiga keadaan:

  | Nilai | Dev | Production |
  |---|---|---|
  | tidak diisi | tidak dibangun | dibangun bila aman (lihat pemindaian `env()`) |
  | `false` | tidak dibangun | tidak dibangun, tanpa kabar |
  | `true` | dibangun | dibangun, pemindaian dilewati |

- **Pemindaian `env()`**. Ini tambahan di luar permintaan harfiah, karena tanpanya fitur
  ini bisa merusak situs diam-diam: sesudah konfigurasi di-cache Laravel tidak memuat
  `.env`, jadi `env()` di luar berkas config mengembalikan `null`. Sebelum membangun,
  jamu membaca berkas `.php` di `app`, `routes`, dan `resources/views`. Bila ada
  panggilan `env(`, cache tidak dibangun dan sebabnya disebut dengan berkas dan baris:
  `jamu: cache Laravel tidak dibangun: app/Layanan/Bayar.php baris 3 memanggil env() di
  luar berkas config ...`. `jamu check` mencatat hal yang sama sebagai `catatan`, bukan
  `MASALAH`. `optimize = true` memaksa.
- **Batas waktu 60 detik** (`supervise::run_once`). Yang melewatinya dimatikan bersama
  anak prosesnya. Permintaan berhenti (Ctrl+C, SIGTERM) tidak menunggu batas itu:
  penangan sinyal dipasang sebelum cache dibangun.
- **Gagal tidak menghalangi situs**. Status bukan nol, batas waktu, atau PHP CLI yang
  tidak ketemu: situs tetap dinyalakan dengan cache yang ada, dan 15 baris terakhir
  keluaran artisan ditampilkan dengan awalan `[optimize]`. Pesannya menyebut
  `optimize = false`. Bila berhasil, keluaran artisan tidak ditampilkan; cukup satu baris
  `jamu: cache Laravel dibangun (\`php artisan optimize\`, 0,3 detik)`.
- **Dev**. Cache tidak dibangun. Bila `bootstrap/cache/config.php` tertinggal (misalnya
  project disalin dari server), `serve` dan `check` menyebutnya, karena `.env` tidak lagi
  terbaca dan itu membingungkan justru di laptop.
- Catatan lama "konfigurasi belum di-cache" di `check` dan `serve` dihapus; tidak ada
  lagi yang perlu dikerjakan pengguna.

### Aturan pemindaian `env()`

Heuristik baris demi baris, bukan pengurai PHP. `env(` dihitung kecuali didahului huruf,
angka, `_`, `>`, `:`, `$`, atau `@`, dan kecuali barisnya komentar (`//`, `*`, `/*`, `#`
selain `#[`). Jadi `getenv(`, `->env(`, `::env(`, `$env(`, `my_env(`, dan direktif Blade
`@env(` tidak dihitung.

Yang lolos dari heuristik ini, dua arah:

- **Dihitung padahal aman**: `env(` di dalam string atau di tengah komentar blok yang
  barisnya tidak diawali `*`. Akibatnya cache tidak dibangun dan sebabnya disebut; jalan
  keluarnya `optimize = true`. Salah ke arah ini tidak merusak apa-apa.
- **Tidak dihitung padahal berbahaya**: `env()` di dalam paket `vendor` (tidak dipindai;
  paket yang benar tidak melakukannya), dan di folder di luar tiga folder itu (misalnya
  `database`, `bootstrap`, atau folder modul buatan sendiri).

### Hal yang perlu diketahui sebelum memakainya

- **Konfigurasi dan rute dibekukan sampai jamu dinyalakan ulang.** Ini perubahan
  perilaku. Sebelumnya, di server tanpa cache, `git pull` langsung terbaca. Sekarang
  rute dan config yang lama tetap dipakai sampai `jamu` dinyalakan ulang atau
  `jamu php artisan optimize` dijalankan. Di container hal ini tidak terasa (deploy =
  container baru); di VPS dengan `--domain` perlu diingat. Sesudah `artisan optimize`
  dijalankan tangan, php-fpm membaca berkas barunya dalam beberapa detik (opcache
  memeriksa ulang cap waktu berkas).
- `bootstrap/cache` harus bisa ditulis oleh pengguna yang menjalankan jamu. Resep
  Dockerfile di README (`COPY --chown=www-data:www-data`) sudah memenuhinya. Bila tidak
  bisa ditulis, artisan gagal dan jalur "gagal tidak menghalangi situs" yang berlaku.
- Waktu start bertambah selama `artisan optimize` berjalan: 0,3 detik pada Laravel baru
  di sandbox.
- `artisan optimize` juga menjalankan perintah `optimize` milik paket (Filament,
  Livewire, dan lain-lain mendaftarkannya). Itu yang dimaui, tetapi berarti waktu dan
  kegagalannya bergantung pada paket yang terpasang.
- Di Windows `kill_tree` hanya mematikan proses artisan itu sendiri, bukan cucunya. Tes
  "artisan menggantung" hanya dijalankan di Linux.

### Hasil uji

`tests/e2e.sh`: 321 pemeriksaan di jalur php-fpm dan 318 di jalur `php-cgi`, semuanya
lulus di sandbox. 45 tes unit. Yang baru:

- Dengan artisan tiruan (`grup_check`): cache dibangun sebelum `jamu: siap`; artisan
  dipanggil tepat sekali; dev tidak membangun dan menyebut cache yang tertinggal;
  `optimize = false` diam; artisan gagal (situs tetap siap, pesan artisan tampil); artisan
  menggantung (situs belum dinyalakan, jamu berhenti dalam 5 detik saat diminta, proses
  artisan ikut mati); `env()` hanya di config (dibangun); `env()` di `app` (tidak
  dibangun, sebabnya disebut, `check` mencatatnya); `optimize = true` memaksa; di luar
  Laravel `optimize = true` diabaikan dengan kabar.
- Dengan Laravel asli (`grup_laravel`): dev tidak meninggalkan cache; di
  `--behind-proxy` cache konfigurasi dan rute benar-benar ada, lalu `/`, `/up`, basis
  data, sesi, rute closure, pekerja antrean, dan 404 tetap benar.

CI (commit `2fce9a3`): Linux, Windows 8.5, image, dan supervisor lulus di percobaan
pertama. Windows 8.4 gagal sekali di `tests/windows_stop.py` (situs PHP biasa, bukan jalur
Laravel): permintaan pertama sesudah `jamu: siap` ditolak sambungannya. Job yang sama lulus
saat dijalankan ulang tanpa perubahan kode. Sebabnya belum diketahui; `jamu: siap` baru
dicetak sesudah port menerima sambungan, jadi ini perlu dicari bila muncul lagi.

### Keputusan yang menunggu owner

1. Menggabungkan pekerjaan ini ke `main` (image `:8.4` terbit ulang dari sana).
2. Worker mode: tidak berubah dari bagian 24.

## Sumber

- WordPress, deteksi server: `wp-includes/vars.php`, `wp-includes/functions.php`, `wp-admin/includes/misc.php` di https://github.com/WordPress/WordPress
- php-src: `sapi/cgi/cgi_main.c` dan `sapi/fpm/fpm/fpm_main.c` di https://github.com/php/php-src
- Plugin yang diperiksa: wp-super-cache, w3-total-cache, wp-rocket, lscache_wp, webp-express, contact-form-7, really-simple-ssl, dan h5bp/server-configs-apache (GitHub)
- certmagic 0.25.6 (`acmeissuer.go`, `async.go`) dan acmez 3.1.7: pesan log sertifikat dan percobaan ulang lewat staging, https://github.com/caddyserver/certmagic
- msmtp 1.8: https://marlam.de/msmtp/msmtp.html
- FrankenPHP di Windows: https://dunglas.dev/2026/03/windows-support-for-frankenphp-its-finally-alive/ dan https://github.com/php/frankenphp/releases/tag/v1.12.0
- static-php-cli: https://packagist.org/packages/crazywhalecc/static-php-cli
- MailCrab: https://github.com/tweedegolf/mailcrab
- libunftp: https://docs.rs/crate/libunftp/latest
- fastcgi-client: https://docs.rs/crate/fastcgi-client/latest
- Pasir: https://www.thedroptimes.com/node/51012 dan https://rust-digger.code-maven.com/crates/pasir
- OxPHP: https://awesome.ecosyste.ms/projects/github.com%2Foxphp%2Foxphp
- Rapira: https://www.besthub.dev/articles/rapira-rust-powered-php-server-hits-111k-rps-10x-faster-than-php-fpm-a13f541ee993
- Laragon: https://laragon.org/docs/faq.html dan https://laragon.org/pricing
- Herd: https://herd.laravel.com/
- PIE: https://php.github.io/pie
- StaticPHP, build glibc: https://static-php.github.io/v2-docs/en/guide/build-with-glibc.html
- ionCube Loader: https://www.ioncube.com/loaders.php
- EnvKit: https://laravel-news.com/envkit-a-local-development-stack-for-laravel-on-windows-and-macos
- ServBay: https://servbay.com/features
- Azure Artifact Signing: https://learn.microsoft.com/azure/artifact-signing/quickstart
- Tanggal rilis crate: API crates.io, diambil 13 Nov 2026
- mod_md: https://httpd.apache.org/docs/2.4/mod/mod_md.html
- Modul ACME nginx: https://nginx.org/en/docs/http/ngx_http_acme_module.html dan https://blog.nginx.org/?p=74612
- Masa berlaku sertifikat Let's Encrypt: https://letsencrypt.org/2025/12/02/from-90-to-45.html dan https://letsencrypt.org/docs/cert-lifetimes/
- Aturan multisite WordPress: `wp-admin/includes/network.php` di https://github.com/WordPress/WordPress
- HTTP/3 di Traefik: https://doc.traefik.io/traefik/v3.5/reference/install-configuration/entrypoints/
- HTTP/3 di Dokploy: https://dokploy.com/blog/v0-20-0-watch-pa dan https://github.com/Dokploy/dokploy/issues/3028
- CA lokal Caddy: https://caddyserver.com/docs/automatic-https
- Caddy dan `.htaccess`: https://caddy.community/t/wordpress-multisite-htaccess-rewrites/12329
- Redis di Windows: https://redis.io/blog/use-redis-natively-on-windows-with-memurai.md dan https://redisson.pro/blog/redis-alternatives.html
- Mode klasik FrankenPHP vs php-fpm: https://tideways.com/profiler/blog/testing-if-franken-php-classic-mode-is-faster-and-more-scalable-than-php-fpm
- Klaim worker mode 2,5 kali: https://deploynix.io/blog/php-fpm-vs-laravel-octane-on-deploynix-real-world-performance-comparison
