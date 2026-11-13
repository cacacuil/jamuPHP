# JamuPHP: handoff v2

Tanggal: 13 November 2026, revisi 8. Menggantikan handoff v1 ("XAMPP berbasis Rust").
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
| PHP, Windows | Pool `php-cgi.exe` | Satu proses melayani satu request; `PHP_FCGI_CHILDREN` tidak berlaku di Windows. Supervisor menjalankan N proses di port loopback, membagi request, dan menghidupkan ulang proses yang keluar setelah `PHP_FCGI_MAX_REQUESTS`. |
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
php  = "8.2"
root = "public"

[php.ini]
memory_limit = "256M"

[[worker]]
name = "queue"
run  = "php artisan queue:work --tries=3"
```

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
| H7 | Plugin cache tidak terpasang atau salah atur | Setiap pengunjung menjalankan PHP | Cache halaman disediakan platform, bukan diserahkan ke plugin. |

## 10. Roadmap dan syarat lulus

Ukuran: K kecil, S sedang, B besar. Ini perkiraan kasar, bukan janji waktu.

**Fase 0, spike (S).** Membuktikan asumsi paling berisiko dengan kode sekali buang.
- S1: supervisor Rust menyalakan Caddy dan php-fpm ondemand di Linux dari hasil deteksi
  project. Laravel baru dan instalasi WordPress jalan, termasuk upload besar, dengan
  sertifikat dari staging Let's Encrypt pada domain sungguhan.
- S2: hal yang sama di Windows dengan Caddy dan pool php-cgi dari zip resmi.
- S3: image `php:*-fpm` + binary sebagai PID 1, satu situs sungguhan di Dokploy.
- S4: ulangi pengukuran bagian 5 dengan MySQL dan plugin cache halaman, tiga kali jalan.
  Hanya bila owner memilih server sendiri: `.htaccess` untuk empat kasus (WordPress,
  Laravel, blok tolak Contact Form 7, aturan WP Super Cache), diuji diferensial terhadap
  Apache.
- Lulus bila keempatnya jalan. Bila S2 macet di Windows, evaluasi nginx untuk Windows atau
  FrankenPHP sebagai lapisan web desktop.

**Fase 1, core headless (B).** Deteksi project, perintah `serve`, `.htaccess` tahap 1,
pengelolaan pool PHP, worker, penjadwal, log, shutdown yang rapi, penerus `php` dan
`composer` yang memilih versi sesuai project. Termasuk tiga hal kecil yang mahal bila
disusulkan: extension `redis` di paket PHP, pencocokan host dengan wildcard, dan jenis
upstream reverse proxy HTTP. Aturan multisite WordPress masuk korpus uji.
- Lulus bila `laravel new` lalu `serve` jalan tanpa file config di Linux dan Windows, tes
  bagian 8 hijau di kedua OS, dan uji diferensial tahap 1 tanpa selisih.

**Fase 2, production (S).** Dua mode: server mandiri dengan SSL sah bawaan lewat mod_md
(tambah domain, pemeriksaan DNS, reload saat `renewed`, status sertifikat), dan container
di belakang Traefik. Lalu healthcheck, kepercayaan proxy, mail mode teruskan, wp-cron, log
JSON, non-root, perintah `check`. Uji SSL terhadap staging Let's Encrypt dengan domain
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
- Pengukuran bagian 5 belum diulang dan belum memakai MySQL.
- Konkurensi diukur dengan beban jenuh; uji dengan laju tetap di bawah titik jenuh belum
  dilakukan. HTTP/2, HTTP/3, dan nginx lewat HTTPS belum diukur.
- Cara membatasi antrean PHP per situs di Caddy (dan jawaban 503 cepat) belum diuji.
- Cache halaman tingkat platform untuk Caddy: modul mana, dan aturan pengecualian untuk
  pengguna login dan keranjang belanja.
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
3. S2: Windows dengan Caddy dan pool php-cgi.
4. S4: ulangi pengukuran dengan MySQL dan cache halaman.

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
2. S2 (Windows) dan S4 (ukur ulang dengan MySQL dan cache halaman).

## Sumber

- WordPress, deteksi server: `wp-includes/vars.php`, `wp-includes/functions.php`, `wp-admin/includes/misc.php` di https://github.com/WordPress/WordPress
- php-src: `sapi/cgi/cgi_main.c` dan `sapi/fpm/fpm/fpm_main.c` di https://github.com/php/php-src
- Plugin yang diperiksa: wp-super-cache, w3-total-cache, wp-rocket, lscache_wp, webp-express, contact-form-7, really-simple-ssl, dan h5bp/server-configs-apache (GitHub)
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
