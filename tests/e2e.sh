#!/usr/bin/env bash
# Uji end-to-end binary jamu: PHP biasa, Laravel, WordPress.
# Pemakaian: JAMU=... LARAVEL_DIR=... WP_DIR=... [GRUP=...] bash tests/e2e.sh
set -u

JAMU="${JAMU:-./target/release/jamu}"
# Path mutlak: beberapa tes menjalankan jamu dari dalam folder situs.
JAMU="$(cd "$(dirname "$JAMU")" && pwd)/$(basename "$JAMU")"
here="$(cd "$(dirname "$0")" && pwd)"
case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) windows=1;; *) windows=0;; esac
if [ "$windows" = 1 ]; then PHP_PROC="${PHP_PROC:-php-cgi.exe}"; else PHP_PROC="${PHP_PROC:-php-fpm}"; fi

work=$(mktemp -d "${TMPDIR:-/tmp}/jamu-e2e.XXXXXX")
# Di Git Bash, path gaya Windows (C:/...) dimengerti bash maupun program Windows asli.
[ "$windows" = 1 ] && work=$(cygpath -m "$work") && here=$(cygpath -m "$here")
lulus=0; gagal=0; grup_gagal=0
JPID=""

# Curl tanpa proxy, timeout umum 60 detik.
cu() { curl --noproxy '*' -s --max-time 60 "$@"; }
kode() { cu -o /dev/null -w '%{http_code}' "$@"; }
# Header respons, tanpa CR, huruf kecil.
hdr() { cu -o /dev/null -D - "$@" | tr -d '\r' | tr 'A-Z' 'a-z'; }

# Jumlah proses dengan nama tertentu (OS-spesifik). Di Windows nama harus lengkap dengan .exe.
count_procs() {
  if [ "$windows" = 1 ]; then
    tasklist //FI "IMAGENAME eq $1" //NH | grep -c -i "$1"
  else
    pgrep -c -f "$1"
  fi
}

# Matikan satu proses dengan nama tertentu secara paksa (OS-spesifik).
kill_one() {
  if [ "$windows" = 1 ]; then
    local pid
    pid=$(tasklist //FI "IMAGENAME eq $1" //FO CSV //NH | head -1 | cut -d, -f2 | tr -d '"')
    case "$pid" in ''|*[!0-9]*) return 1;; esac
    taskkill //F //PID "$pid" >/dev/null
  else
    # Hanya anak langsung dari jamu yang sedang diuji, bukan proses lain yang kebetulan bernama sama.
    local pid
    pid=$(pgrep -P "$JPID" -f "$1" | head -1)
    [ -n "$pid" ] && kill -9 "$pid"
  fi
}

# Cek satu hasil: cek NAMA DIDAPAT HARAPAN [DETAIL]
cek() {
  if [ "$2" = "$3" ]; then
    echo "ok     $1"; lulus=$((lulus+1))
  else
    echo "GAGAL  $1: ${4:-dapat [$2], harap [$3]}"
    gagal=$((gagal+1)); grup_gagal=$((grup_gagal+1))
  fi
}
# Isi jawaban tidak boleh memuat TEKS: bocor NAMA TEKS [opsi curl...] URL
bocor() {
  local nama=$1 teks=$2 isi; shift 2
  isi=$(cu --path-as-is "$@" | tr -d '\0')
  cek "$nama" "$(has "$isi" "$teks")" no "jawaban memuat [$teks]: $(printf %s "$isi" | head -c 80)"
}
# Content-Type tanpa parameter (charset).
jenis() { hdr "$1" | grep '^content-type:' | sed 's/^content-type: *//; s/;.*//'; }
# ya/tidak untuk perintah; has/akhiran untuk teks.
ya() { if "$@"; then echo yes; else echo no; fi; }
has() { case "$1" in *"$2"*) echo yes;; *) echo no;; esac; }
akhiran() { case "$1" in *"$2") echo yes;; *) echo no;; esac; }

# Jalankan jamu di latar belakang, tunggu baris "jamu: siap" (maks 40 detik).
start_jamu() { # DIR PORT LOG [OPSI...]
  local dir=$1 port=$2 log=$3; shift 3
  "$JAMU" serve "$dir" --port "$port" "$@" < /dev/null > /dev/null 2> "$log" &
  JPID=$!
  for _ in $(seq 1 80); do
    grep -q 'jamu: siap' "$log" 2>/dev/null && return 0
    sleep 0.5
  done
  return 1
}
# Hentikan satu proses jamu. Di Git Bash, `kill` untuk program Windows asli juga mematikan
# semua "keturunan" yang ia temukan lewat PID induk. PID di Windows dipakai ulang, jadi
# proses yang tidak ada hubungannya bisa ikut mati, termasuk runner CI. Karena itu di
# Windows jamu dimatikan lewat PID Windows-nya; anak-anaknya ikut mati lewat Job Object.
matikan() { # PID versi bash
  local w=""
  [ "$windows" = 1 ] && w=$(cat "/proc/$1/winpid" 2>/dev/null)
  case "$w" in
    ''|*[!0-9]*)
      [ "$windows" = 1 ] && echo "catatan: PID Windows tidak terbaca; memakai kill"
      kill "$1" 2>/dev/null;;
    *) if tasklist //FI "PID eq $w" //NH 2>/dev/null | grep -qi 'jamu'; then
         taskkill //F //PID "$w" >/dev/null 2>&1
       else
         echo "catatan: PID Windows $w bukan jamu; memakai kill"
         kill "$1" 2>/dev/null
       fi;;
  esac
}
stop_jamu() { [ -n "$JPID" ] || return 0; matikan "$JPID"; wait "$JPID" 2>/dev/null; JPID=""; }
# Bila skrip ini berhenti di tengah jalan, jamu yang sedang diuji tidak boleh tertinggal.
trap stop_jamu EXIT

# Ulangi GET sampai 200, maks 5 detik.
tunggu_200() {
  for _ in $(seq 1 10); do
    [ "$(kode "$1")" = 200 ] && return 0
    sleep 0.5
  done
  return 1
}

# Tampilkan log jamu bila grup ini ada yang gagal.
akhir_grup() { # LOG
  if [ "$grup_gagal" -gt 0 ]; then
    echo "--- 30 baris terakhir log: $1 ---"
    tail -30 "$1" 2>/dev/null
  fi
  grup_gagal=0
}

grup_php() {
  echo "== php biasa =="
  local site="$work/Situs Saya é" htd log B=http://127.0.0.1:8101 d h
  htd="$site/htdocs"; log="$work/php.log"
  mkdir -p "$htd/.git"
  printf '%s\n' '<?php echo "halo dari php ", PHP_VERSION;' > "$htd/index.php"
  printf '%s\n' 'RAHASIA=1' > "$htd/.env"
  printf '%s\n' '[core]' > "$htd/.git/config"
  cat > "$htd/upload.php" <<'EOF'
<?php $f = $_FILES['berkas'] ?? null; if (!$f || $f['error']) { http_response_code(400); exit('gagal'); } echo $f['size'], ' ', hash_file('sha256', $f['tmp_name']);
EOF
  cat > "$htd/alir.php" <<'EOF'
<?php header('Content-Type: text/plain'); for ($i = 1; $i <= 4; $i++) { echo "potongan $i\n"; @ob_flush(); flush(); usleep(500000); }
EOF
  printf '%s\n' '<?php error_log("JEJAK-LOG-E2E"); echo "ok";' > "$htd/catat.php"
  cat > "$htd/lambat.php" <<'EOF'
<?php usleep(700000); echo 'selesai';
EOF
  # Bagian 8 HANDOFF: berkas untuk uji jalur, dotfile, httpoxy, dan berkas statis.
  mkdir -p "$htd/.well-known" "$htd/sub"
  printf '%s\n' 'RAHASIA-LUAR' > "$site/di-luar.txt"
  printf '%s\n' 'RAHASIA-HTACCESS' > "$htd/.htaccess"
  printf '%s\n' 'RAHASIA-USERINI' > "$htd/.user.ini"
  printf '%s\n' 'boleh' > "$htd/.well-known/uji.txt"
  printf '%s\n' 'isi' > "$htd/sub/berkas-di-sub.txt"
  printf '%s\n' '<?php echo 6 * 7, "-jalan";' > "$htd/gambar.jpg"
  cat > "$htd/lingkungan.php" <<'EOF'
<?php header('Content-Type: text/plain');
echo 'https=', $_SERVER['HTTPS'] ?? '-', ' addr=', $_SERVER['REMOTE_ADDR'], ' proxy=', $_SERVER['HTTP_PROXY'] ?? '-', '|', getenv('HTTP_PROXY') ?: '-', "\n";
EOF
  printf '%s\n' 'console.log(1);' > "$htd/a.js"
  printf '%s\n' 'export default 1;' > "$htd/a.mjs"
  printf '%s\n' 'export default 2;' > "$htd/BESAR.MJS"
  printf '%s\n' '<svg xmlns="http://www.w3.org/2000/svg"/>' > "$htd/a.svg"
  printf '%s\n' '{"a":1}' > "$htd/a.json"
  printf '%s\n' '{"name":"uji"}' > "$htd/a.webmanifest"
  printf '%s\n' 'teks biasa' > "$htd/a.txt"
  printf '%s\n' '<!doctype html><title>uji</title>' > "$htd/a.html"
  printf '\x89PNG\r\n\x1a\n' > "$htd/a.png"
  printf 'wOF2\0\1\0\0' > "$htd/a.woff2"
  printf 'RIFF\x1a\0\0\0WEBPVP8 ' > "$htd/a.webp"
  printf '\0\0\1\0\1\0' > "$htd/a.ico"
  printf '\0asm\1\0\0\0' > "$htd/a.wasm"
  for i in $(seq 1 300); do echo "body { color: #333; margin: 0; } /* aturan gaya nomor $i */"; done > "$htd/gaya.css"
  head -c 100000000 /dev/urandom > "$work/besar-100.bin"
  head -c 200000000 /dev/zero > "$work/besar-200.bin"
  h=$(sha256sum "$work/besar-100.bin" | cut -d' ' -f1)

  local awal_caddy awal_php
  local nama_caddy=caddy
  [ "$windows" = 1 ] && nama_caddy=caddy.exe
  awal_caddy=$(count_procs "$nama_caddy"); awal_php=$(count_procs "$PHP_PROC")

  # 1. deteksi proyek
  d=$("$JAMU" detect "$site" 2>&1)
  cek "detect: document root htdocs" "$(has "$d" htdocs)" yes

  if ! start_jamu "$site" 8101 "$log"; then
    cek "jamu siap (php)" no yes "jamu tidak menampilkan 'jamu: siap' dalam 40 detik"
    stop_jamu; akhir_grup "$log"; return
  fi

  # 2. halaman utama
  cek "/ 200" "$(kode "$B/")" 200
  cek "/ memuat 'halo dari php'" "$(has "$(cu "$B/")" 'halo dari php')" yes
  # 3. berkas tersembunyi tidak boleh terlayani
  cek "/.env 404" "$(kode "$B/.env")" 404
  cek "/.git/config 404" "$(kode "$B/.git/config")" 404
  cek "/.htaccess 404" "$(kode "$B/.htaccess")" 404
  cek "/.user.ini 404" "$(kode "$B/.user.ini")" 404
  cek "/.well-known/uji.txt 200" "$(kode "$B/.well-known/uji.txt")" 200
  # jalur: tidak boleh keluar dari document root, byte nol tidak boleh membocorkan sumber
  local u
  for u in /../di-luar.txt /..%2fdi-luar.txt /%2e%2e/di-luar.txt /..%5cdi-luar.txt /sub/../../di-luar.txt \
    /sub/..%2f..%2fdi-luar.txt //../di-luar.txt '/..\di-luar.txt' /....//di-luar.txt; do
    bocor "jalur $u tidak keluar dari root" RAHASIA-LUAR "$B$u"
  done
  for u in /index.php%00.css /gaya.css%00.php /index.php%00 /%00; do
    bocor "byte nol $u tidak membocorkan sumber" '<?php' "$B$u"
  done
  # hanya berkas .php yang ada di disk yang dijalankan
  bocor "/gambar.jpg/y.php tidak dijalankan" 42-jalan "$B/gambar.jpg/y.php"
  bocor "/gambar.jpg/index.php tidak dijalankan" 42-jalan "$B/gambar.jpg/index.php"
  bocor "/gambar.jpg%00.php tidak dijalankan" 42-jalan "$B/gambar.jpg%00.php"
  # tanpa daftar isi direktori
  bocor "/sub/ tanpa daftar isi" berkas-di-sub "$B/sub/"
  # httpoxy: header Proxy dari pengunjung tidak boleh menjadi HTTP_PROXY
  bocor "header Proxy dibuang (httpoxy)" jahat -H 'Proxy: http://jahat.invalid:1' "$B/lingkungan.php"
  # mode dev tidak di belakang proxy: header terusan tidak dipercaya
  local l
  l=$(cu -H 'X-Forwarded-Proto: https' -H 'X-Forwarded-For: 203.0.113.9' "$B/lingkungan.php")
  cek "dev: X-Forwarded-Proto diabaikan" "$(has "$l" 'https=-')" yes "$l"
  cek "dev: X-Forwarded-For diabaikan" "$(has "$l" 'addr=127.0.0.1')" yes "$l"
  # berkas statis: jenis isi, ETag, Range, HEAD
  cek "jenis .css" "$(jenis "$B/gaya.css")" text/css
  cek "jenis .js" "$(jenis "$B/a.js")" text/javascript
  cek "jenis .mjs" "$(jenis "$B/a.mjs")" text/javascript
  cek "jenis .MJS (huruf besar)" "$(jenis "$B/BESAR.MJS")" text/javascript
  cek "jenis .svg" "$(jenis "$B/a.svg")" image/svg+xml
  cek "jenis .json" "$(jenis "$B/a.json")" application/json
  cek "jenis .webmanifest" "$(jenis "$B/a.webmanifest")" application/manifest+json
  cek "jenis .txt" "$(jenis "$B/a.txt")" text/plain
  cek "jenis .html" "$(jenis "$B/a.html")" text/html
  cek "jenis .png" "$(jenis "$B/a.png")" image/png
  cek "jenis .woff2" "$(jenis "$B/a.woff2")" font/woff2
  cek "jenis .webp" "$(jenis "$B/a.webp")" image/webp
  cek "jenis .ico" "$(has "$(jenis "$B/a.ico")" icon)" yes "$(jenis "$B/a.ico")"
  cek "jenis .wasm" "$(jenis "$B/a.wasm")" application/wasm
  # Jenis isi di atas hanya untuk berkas statis: jawaban PHP membawa jenisnya sendiri.
  cek "jawaban PHP untuk /tidak-ada.js tetap text/html" "$(jenis "$B/tidak-ada.js")" text/html
  cek "jawaban PHP hanya punya satu Content-Type" "$(hdr "$B/tidak-ada.js" | grep -c '^content-type:')" 1
  local etag
  etag=$(cu -o /dev/null -D - "$B/gaya.css" | tr -d '\r' | grep -i '^etag:' | sed 's/^[^:]*: *//')
  cek "gaya.css punya ETag" "$([ -n "$etag" ] && echo yes || echo no)" yes
  cek "If-None-Match 304" "$(kode -H "If-None-Match: $etag" "$B/gaya.css")" 304
  cek "Range 206" "$(kode -r 0-9 "$B/gaya.css")" 206
  cek "Range isi 10 byte pertama" "$(cu -r 0-9 "$B/gaya.css")" 'body { col'
  cek "HEAD gaya.css 200" "$(kode -I "$B/gaya.css")" 200
  # 4. kompresi
  cek "gaya.css zstd" "$(has "$(hdr -H 'Accept-Encoding: zstd' "$B/gaya.css")" 'content-encoding: zstd')" yes
  cek "a.png tidak dikompresi" "$(has "$(hdr -H 'Accept-Encoding: gzip' "$B/a.png")" 'content-encoding')" no
  cek "gaya.css gzip" "$(has "$(hdr -H 'Accept-Encoding: gzip' "$B/gaya.css")" 'content-encoding: gzip')" yes
  # 5-6. unggah berkas
  local r
  r=$(cu -o "$work/up.out" -w '%{http_code}' -F "berkas=@$work/besar-100.bin" "$B/upload.php")
  cek "upload 100 MB kode" "$r" 200
  cek "upload 100 MB isi" "$(cat "$work/up.out")" "100000000 $h"
  cek "upload 200 MB 413" "$(kode -F "berkas=@$work/besar-200.bin" "$B/upload.php")" 413
  rm -f "$work/besar-100.bin" "$work/besar-200.bin" "$work/up.out"
  # 7. streaming: byte pertama cepat, total lambat
  local t ttfb total
  t=$(cu -o /dev/null -w '%{time_starttransfer} %{time_total}' "$B/alir.php")
  ttfb=${t% *}; total=${t#* }
  cek "stream: ttfb < 1 s" "$(awk -v a="$ttfb" 'BEGIN{print (a<1.0)?"yes":"no"}')" yes "ttfb=${ttfb}s"
  cek "stream: total > 1.5 s" "$(awk -v a="$total" 'BEGIN{print (a>1.5)?"yes":"no"}')" yes "total=${total}s"
  # perubahan kode harus langsung terlihat, tanpa menunggu opcache
  printf '%s\n' '<?php echo "versi satu";' > "$htd/ubah.php"
  cek "ubah.php versi satu" "$(cu "$B/ubah.php")" "versi satu"
  sleep 1.1
  printf '%s\n' '<?php echo "versi dua!";' > "$htd/ubah.php"
  cek "ubah.php langsung versi dua" "$(cu "$B/ubah.php")" "versi dua!"
  # catatan galat PHP harus sampai ke log jamu
  cek "catat.php 200" "$(kode "$B/catat.php")" 200
  local ada=no
  for _ in 1 2 3 4 5 6; do grep -q JEJAK-LOG-E2E "$log" && { ada=yes; break; }; sleep 0.5; done
  cek "error_log() PHP muncul di log jamu" "$ada" yes
  # 8. konkurensi: 8 permintaan paralel
  local pids="" i ok=0
  for i in 1 2 3 4 5 6 7 8; do
    cu -o "$work/par$i.txt" -w '%{http_code}' "$B/lambat.php" > "$work/par$i.kode" &
    pids="$pids $!"
  done
  wait $pids
  for i in 1 2 3 4 5 6 7 8; do
    [ "$(cat "$work/par$i.kode")" = 200 ] && [ "$(cat "$work/par$i.txt")" = selesai ] && ok=$((ok+1))
  done
  cek "8 paralel lambat.php" "$ok" 8
  # 9. hanya loopback: IP non-loopback harus ditolak
  local ip rc
  ip=$(hostname -I 2>/dev/null | tr ' ' '\n' | grep -v ':' | grep -v '^127\.' | grep . | head -1)
  if [ "$windows" = 1 ] || [ -z "$ip" ]; then
    echo "lewati  loopback saja: tidak ada IP non-loopback yang terdeteksi"
  else
    curl --noproxy '*' -s --max-time 3 -o /dev/null "http://$ip:8101/"; rc=$?
    cek "loopback saja (IP $ip)" "$([ "$rc" -ne 0 ] && echo gagal-konek || echo terhubung)" gagal-konek
  fi
  # 10. port bentrok: instance kedua harus keluar dengan error
  "$JAMU" serve "$site" --port 8101 2> "$work/konflik.log" &
  local p2=$! mati=no
  for _ in $(seq 1 20); do
    kill -0 "$p2" 2>/dev/null || { mati=yes; break; }
    sleep 0.5
  done
  if [ "$mati" = yes ]; then wait "$p2"; rc=$?; else matikan "$p2"; wait "$p2" 2>/dev/null; rc=0; fi
  cek "port bentrok: keluar non-nol dalam 10 s" "$([ "$mati" = yes ] && [ "$rc" -ne 0 ] && echo yes || echo no)" yes
  cek "port bentrok: log menyebut 8101" "$(has "$(cat "$work/konflik.log")" 8101)" yes
  # 11. matikan worker PHP: layanan harus pulih
  kill_one "$PHP_PROC"; rc=$?
  cek "worker PHP ditemukan dan dimatikan" "$([ "$rc" -eq 0 ] && echo yes || echo no)" yes
  cek "setelah worker mati, / 200 <= 5 s" "$(tunggu_200 "$B/" && echo 200 || echo gagal)" 200
  # 12. matikan Caddy: layanan harus pulih
  kill_one "$nama_caddy"; rc=$?
  cek "caddy ditemukan dan dimatikan" "$([ "$rc" -eq 0 ] && echo yes || echo no)" yes
  cek "setelah caddy mati, / 200 <= 5 s" "$(tunggu_200 "$B/" && echo 200 || echo gagal)" 200
  # 13. berhenti: proses yang dibuat tes harus hilang
  stop_jamu
  for _ in $(seq 1 10); do
    [ "$(count_procs "$nama_caddy")" = "$awal_caddy" ] && [ "$(count_procs "$PHP_PROC")" = "$awal_php" ] && break
    sleep 0.5
  done
  cek "stop: proses caddy kembali ke awal" "$(count_procs "$nama_caddy")" "$awal_caddy"
  cek "stop: proses PHP kembali ke awal" "$(count_procs "$PHP_PROC")" "$awal_php"
  akhir_grup "$log"
}

grup_proxy() {
  echo "== di belakang proxy =="
  local site="$work/proxy" log="$work/proxy.log" B=http://127.0.0.1:8104 l
  mkdir -p "$site"
  cat > "$site/index.php" <<'EOF'
<?php header('Content-Type: text/plain');
echo 'https=', $_SERVER['HTTPS'] ?? '-', ' addr=', $_SERVER['REMOTE_ADDR'], ' proxy=', $_SERVER['HTTP_PROXY'] ?? '-', ' limit=', ini_get('memory_limit'), "\n";
EOF
  printf '%s\n' '# RAHASIA-TOML' '[ini]' 'memory_limit = "200M"' > "$site/jamu.toml"
  if ! start_jamu "$site" 8104 "$log" --behind-proxy; then
    cek "jamu siap (proxy)" no yes "jamu tidak menampilkan 'jamu: siap' dalam 40 detik"
    stop_jamu; akhir_grup "$log"; return
  fi
  l=$(cu "$B/")
  cek "tanpa header terusan: bukan HTTPS" "$(has "$l" 'https=-')" yes "$l"
  l=$(cu -H 'X-Forwarded-Proto: https' -H 'X-Forwarded-For: 203.0.113.9' "$B/")
  cek "proxy jaringan privat: HTTPS=on" "$(has "$l" 'https=on')" yes "$l"
  cek "proxy jaringan privat: alamat pengunjung" "$(has "$l" 'addr=203.0.113.9')" yes "$l"
  # Pengunjung menyelipkan alamat palsu di depan; yang dipakai alamat yang ditambahkan proxy.
  l=$(cu -H 'X-Forwarded-For: 198.51.100.1, 203.0.113.9' "$B/")
  cek "X-Forwarded-For palsu di depan diabaikan" "$(has "$l" 'addr=203.0.113.9')" yes "$l"
  bocor "header Proxy dibuang (httpoxy)" jahat -H 'Proxy: http://jahat.invalid:1' "$B/"
  # jamu.toml ada di document root situs ini: pengaturannya dipakai, filenya tidak tersaji.
  cek "[ini] dari jamu.toml dipakai" "$(has "$(cu "$B/")" 'limit=200M')" yes
  local u
  for u in /jamu.toml /JAMU.TOML /jamu.toml. /jamu.toml%20; do
    bocor "$u tidak tersaji" RAHASIA-TOML "$B$u"
  done
  cek "/jamu.toml 404" "$(kode "$B/jamu.toml")" 404
  stop_jamu
  akhir_grup "$log"
}

# Jalankan jamu yang seharusnya menolak: gagal_start NAMA TEKS DIR [OPSI...]
# Lulus bila ia keluar sendiri dengan kode bukan nol dan pesannya memuat TEKS.
gagal_start() {
  local nama=$1 teks=$2 rc; shift 2
  timeout 20 "$JAMU" "$@" > "$work/tolak.log" 2>&1; rc=$?
  cek "$nama: keluar dengan galat" "$([ "$rc" -ne 0 ] && [ "$rc" -ne 124 ] && echo yes || echo no)" yes "kode keluar $rc"
  cek "$nama: pesan memuat '$teks'" "$(has "$(cat "$work/tolak.log")" "$teks")" yes "$(head -c 300 "$work/tolak.log")"
}

grup_toml() {
  echo "== jamu.toml, pekerja, jamu php =="
  local site="$work/toml" log="$work/toml.log" B=http://127.0.0.1:8105 d r n1 n2 pid baru
  mkdir -p "$site/umum" "$site/salah"
  printf '%s\n' '<?php echo "root-proyek";' > "$site/index.php"
  printf '%s\n' '<?php echo "limit=", ini_get("memory_limit"), " tz=", ini_get("date.timezone");' > "$site/umum/index.php"
  cat > "$site/pekerja.php" <<'EOF'
<?php while (true) { file_put_contents(__DIR__ . '/pekerja.log', getmypid() . ' ' . ini_get('date.timezone') . "\n", FILE_APPEND); usleep(200000); }
EOF
  printf '%s\n' '<?php sleep(8); echo "bangun";' > "$site/umum/tidur.php"
  printf '%s\n' '<?php echo "terima=", strlen(file_get_contents("php://input"));' > "$site/umum/terima.php"
  head -c 5000 /dev/zero > "$work/lima-kb.bin"
  cat > "$site/jamu.toml" <<'EOF'
root = "umum"
timeout = 2

[ini]
memory_limit = "300M"
post_max_size = "1K"
"date.timezone" = "Asia/Jakarta"

[[worker]]
name = "catat"
run = "php pekerja.php"
EOF
  d=$("$JAMU" detect "$site" 2>&1)
  cek "detect: root dari jamu.toml" "$(has "$d" umum)" yes "$d"
  cek "detect: pekerja disebut" "$(has "$d" 'pekerja catat')" yes "$d"

  # yang salah ditolak sebelum apa pun dinyalakan
  printf '%s\n' '<?php echo 1;' > "$site/salah/index.php"
  printf '%s\n' 'rot = "umum"' > "$site/salah/jamu.toml"
  gagal_start "kunci tidak dikenal" rot detect "$site/salah"
  printf '%s\n' 'root = ".."' > "$site/salah/jamu.toml"
  gagal_start "root di luar project" 'di dalam project' detect "$site/salah"
  printf '%s\n' 'php = "7.4"' > "$site/salah/jamu.toml"
  gagal_start "versi PHP tidak cocok" 'PHP 7.4' serve "$site/salah" --port 8106
  r=$(cd "$site/salah" && "$JAMU" php -r 'echo "jalan";' 2>&1)
  cek "jamu php menolak versi yang tidak cocok" "$(has "$r" 'PHP 7.4')" yes "$r"

  if ! start_jamu "$site" 8105 "$log"; then
    cek "jamu siap (toml)" no yes "jamu tidak menampilkan 'jamu: siap' dalam 40 detik"
    stop_jamu; akhir_grup "$log"; return
  fi
  r=$(cu "$B/")
  cek "root dari jamu.toml disajikan" "$(has "$r" 'limit=')" yes "$r"
  cek "[ini] memory_limit dipakai web" "$(has "$r" 'limit=300M')" yes "$r"
  cek "[ini] date.timezone dipakai web" "$(has "$r" 'tz=Asia/Jakarta')" yes "$r"
  # batas ukuran body mengikuti post_max_size yang berlaku: ditolak sebelum sampai ke PHP
  cek "[ini] post_max_size 1K: body 5 KB ditolak 413" "$(kode --data-binary "@$work/lima-kb.bin" "$B/terima.php")" 413
  cek "[ini] post_max_size 1K: body kecil diterima" "$(cu --data-binary 'a=1' "$B/terima.php")" "terima=3"
  # batas waktu satu permintaan: yang menggantung dijawab 504 dan pekerjanya dikembalikan
  r=$(cu -o "$work/tidur.out" -w '%{http_code} %{time_total}' "$B/tidur.php")
  cek "timeout 2 detik: permintaan 8 detik tidak selesai" "$(has "$(cat "$work/tidur.out")" bangun)" no
  cek "timeout 2 detik: dijawab 504" "${r% *}" 504
  cek "timeout 2 detik: diputus sebelum 5 detik" "$(awk -v a="${r#* }" 'BEGIN{print (a<5)?"yes":"no"}')" yes "$r"
  if [ "$PHP_PROC" = php-fpm ]; then
    baru=no
    for _ in $(seq 1 12); do grep -q 'jamu: permintaan GET /tidur.php dihentikan' "$log" && { baru=yes; break; }; sleep 0.5; done
    cek "timeout: pekerja dihentikan dan log menyebut permintaannya" "$baru" yes "$(grep -i 'tidur\|timed' "$log" | head -3)"
  else
    echo "lewati  timeout: menghentikan pekerja hanya di jalur php-fpm"
  fi
  cek "timeout: situs tetap melayani" "$(has "$(cu "$B/")" 'limit=300M')" yes
  # pekerja berjalan, memakai PHP dan pengaturan yang sama
  for _ in $(seq 1 20); do [ -s "$site/pekerja.log" ] && break; sleep 0.5; done
  cek "pekerja berjalan" "$([ -s "$site/pekerja.log" ] && echo yes || echo no)" yes
  cek "pekerja memakai [ini]" "$(has "$(head -1 "$site/pekerja.log")" Asia/Jakarta)" yes "$(head -1 "$site/pekerja.log")"
  # pekerja yang mati dinyalakan ulang
  pid=$(tail -1 "$site/pekerja.log" | cut -d' ' -f1)
  case "$pid" in
    ''|*[!0-9]*) cek "PID pekerja terbaca" "$pid" angka;;
    *) if [ "$windows" = 1 ]; then taskkill //F //PID "$pid" > /dev/null; else kill -9 "$pid"; fi;;
  esac
  baru=no
  for _ in $(seq 1 20); do
    n1=$(tail -1 "$site/pekerja.log" | cut -d' ' -f1)
    [ -n "$n1" ] && [ "$n1" != "$pid" ] && { baru=yes; break; }
    sleep 0.5
  done
  cek "pekerja yang mati dinyalakan ulang" "$baru" yes
  # jamu php dan jamu composer: PHP yang sama, dari folder project
  r=$(cd "$site" && "$JAMU" php -r 'echo ini_get("memory_limit"), " ", ini_get("date.timezone"), " ", extension_loaded("mbstring") ? "mb" : "tanpa-mb";' 2>&1)
  cek "jamu php memakai [ini]" "$(has "$r" 'Asia/Jakarta')" yes "$r"
  cek "jamu php memakai memory_limit dari [ini]" "$(has "$r" '300M')" yes "$r"
  cek "jamu php: ekstensi aktif" "$(has "$r" ' mb')" yes "$r"
  (cd "$site" && "$JAMU" php -r 'exit(7);'); r=$?
  cek "jamu php meneruskan kode keluar" "$r" 7
  r=$(cd "$site" && "$JAMU" composer --version 2>&1)
  cek "jamu composer berjalan" "$(has "$r" 'Composer')" yes "$r"
  # berhenti: pekerja ikut berhenti
  stop_jamu
  n1=$(wc -l < "$site/pekerja.log"); sleep 1; n2=$(wc -l < "$site/pekerja.log")
  cek "stop: pekerja ikut berhenti" "$n1" "$n2"
  akhir_grup "$log"
}

# Aturan khusus WordPress, diuji pada situs tiruan di belakang proxy: berkas cache plugin,
# multisite subdirektori, wp-cron, dan log akses. Plugin sungguhan diuji di grup wordpress.
grup_wp_aturan() {
  echo "== aturan wordpress: cache, multisite, wp-cron =="
  local site="$work/wp-aturan" log="$work/wp-aturan.log" B=http://127.0.0.1:8107 r c
  local cache="$site/wp-content/cache/supercache/situs.test" plugin="$site/wp-content/plugins/wp-super-cache"
  mkdir -p "$cache/halo" "$plugin" "$site/wp-admin" "$site/wp-content/uploads" "$site/wp-content/.git"
  printf '%s\n' '<?php' 'define( "MULTISITE", true );' 'define( "SUBDOMAIN_INSTALL", false );' > "$site/wp-config.php"
  printf '%s\n' '<?php // tiruan' > "$site/wp-load.php"
  printf '%s\n' '<?php header("X-Dari: php"); echo "php:", $_SERVER["REQUEST_URI"];' > "$site/index.php"
  cp "$site/index.php" "$site/wp-admin/index.php"
  printf '%s\n' '<?php echo "CRON-JALAN\n";' > "$site/wp-cron.php"
  printf '%s\n' '<?php' > "$plugin/wp-cache-phase1.php"
  printf '%s\n' '<?php echo "DIEKSEKUSI";' > "$site/wp-content/uploads/x.php"
  echo RAHASIA-GIT > "$site/wp-content/.git/config"
  echo CACHE-HTTP-BERANDA > "$cache/index.html"
  echo CACHE-HTTPS-BERANDA > "$cache/index-https.html"
  echo CACHE-HTTPS-HALO > "$cache/halo/index-https.html"
  if ! start_jamu "$site" 8107 "$log" --behind-proxy; then
    cek "jamu siap (aturan wordpress)" no yes "jamu tidak menampilkan 'jamu: siap' dalam 40 detik"
    stop_jamu; akhir_grup "$log"; return
  fi
  c() { cu -H 'Host: situs.test' "$@"; }
  # berkas cache disajikan Caddy, tiap skema dengan berkasnya sendiri
  cek "cache: GET / lewat HTTP" "$(c "$B/")" CACHE-HTTP-BERANDA
  cek "cache: GET / lewat HTTPS dari proxy" "$(c -H 'X-Forwarded-Proto: https' "$B/")" CACHE-HTTPS-BERANDA
  cek "cache: GET /halo/ lewat HTTPS" "$(c -H 'X-Forwarded-Proto: https' "$B/halo/")" CACHE-HTTPS-HALO
  cek "cache: HEAD / 200" "$(c -I -o /dev/null -w '%{http_code}' "$B/")" 200
  r=$(c -o /dev/null -D - "$B/" | tr -d '\r' | tr 'A-Z' 'a-z')
  cek "cache: tidak lewat PHP" "$(has "$r" 'x-dari: php')" no
  cek "cache: Cache-Control pendek" "$(has "$r" 'cache-control: max-age=3, must-revalidate')" yes "$r"
  cek "cache: jenis isi text/html" "$(has "$r" 'content-type: text/html')" yes "$r"
  # yang tidak boleh menerima cache diteruskan ke PHP
  cek "cache: varian HTTP belum ada -> PHP" "$(c "$B/halo/")" "php:/halo/"
  cek "cache: ada query -> PHP" "$(c "$B/?x=1")" "php:/?x=1"
  cek "cache: pengunjung masuk log -> PHP" "$(c -H 'Cookie: wordpress_logged_in_abc=1' "$B/")" "php:/"
  cek "cache: komentator -> PHP" "$(c -H 'Cookie: comment_author_abc=x' "$B/")" "php:/"
  cek "cache: POST -> PHP" "$(c -X POST "$B/")" "php:/"
  cek "cache: host lain -> PHP" "$(cu -H 'Host: lain.test' "$B/")" "php:/"
  bocor "cache: host tidak lazim tidak membuka berkas cache" CACHE -H 'Host: ../situs.test' "$B/"
  bocor "cache: host huruf besar tidak salah folder" CACHE -H 'Host: SITUS.TEST' "$B/"
  # folder plugin dihapus tanpa dinonaktifkan: berkas cache yang tertinggal tidak disajikan lagi
  mv "$plugin" "$plugin.dihapus"
  cek "cache: plugin dihapus -> PHP" "$(c "$B/")" "php:/"
  mv "$plugin.dihapus" "$plugin"
  cek "cache: plugin kembali -> cache" "$(c "$B/")" CACHE-HTTP-BERANDA
  # multisite subdirektori
  r=$(c -o /dev/null -w '%{http_code} %{redirect_url}' "$B/toko/wp-admin")
  cek "multisite: /toko/wp-admin 301" "${r%% *}" 301
  cek "multisite: ke /toko/wp-admin/" "$(akhiran "${r#* }" /toko/wp-admin/)" yes "$r"
  cek "multisite: /toko/wp-admin/index.php dijalankan" "$(c "$B/toko/wp-admin/index.php")" "php:/toko/wp-admin/index.php"
  cek "multisite: /toko/halaman/ ke index.php" "$(c "$B/toko/halaman/")" "php:/toko/halaman/"
  cek "multisite: PHP di uploads tetap 403" "$(kode "$B/toko/wp-content/uploads/x.php")" 403
  cek "multisite: dotfile tetap 404" "$(kode "$B/toko/wp-content/.git/config")" 404
  bocor "multisite: dotfile tidak bocor" RAHASIA "$B/toko/wp-content/.git/config"
  # log akses: satu baris per permintaan, tanpa pemeriksaan kesehatan
  sleep 0.5
  cek "log akses mencatat permintaan" "$(ya grep -q '\[akses\] 127.0.0.1 GET /toko/halaman/ 200' "$log")" yes
  # Ping hanya ada di jalur php-fpm; php-cgi tidak punya padanannya.
  if [ "$PHP_PROC" = php-fpm ]; then
    cek "ping dijawab" "$(cu "$B/-/jamu/ping")" pong
    sleep 0.5
    cek "log akses tidak mencatat ping" "$(grep -c 'jamu/ping' "$log")" 0
  fi
  # wp-cron tanpa pengunjung (paling lama menunggu satu pergantian menit)
  for _ in $(seq 1 150); do grep -q 'CRON-JALAN' "$log" && break; sleep 0.5; done
  cek "wp-cron dijalankan penjadwal" "$(ya grep -q '\[schedule\] CRON-JALAN' "$log")" yes
  stop_jamu
  akhir_grup "$log"
}

grup_laravel() {
  echo "== laravel =="
  if [ -z "${LARAVEL_DIR:-}" ]; then echo "lewati: laravel (LARAVEL_DIR tidak diset)"; return; fi
  local lar="$work/laravel" log="$work/laravel.log" B=http://127.0.0.1:8102 jar="$work/laravel.jar" s1 s2
  # Salinan, supaya project sumber tidak berubah. Ditambah satu tugas antrean, satu tugas
  # terjadwal, dan jamu.toml yang menyalakan pekerja antrean serta penjadwal di mode dev.
  cp -a "$LARAVEL_DIR" "$lar"
  cat >> "$lar/routes/web.php" <<'EOF'
Route::get('/uji-antre', function () {
    dispatch(
        function () { file_put_contents(storage_path('antre.txt'), 'selesai'); }
    );
    return 'masuk-antrean';
});
EOF
  cat >> "$lar/routes/console.php" <<'EOF'
\Illuminate\Support\Facades\Schedule::call(function () { file_put_contents(storage_path('jadwal.txt'), "x\n", FILE_APPEND); })->everyMinute();
EOF
  cat > "$lar/jamu.toml" <<'EOF'
scheduler = true

[[worker]]
name = "queue"
run = "php artisan queue:work --tries=1"
EOF
  if ! start_jamu "$lar" 8102 "$log"; then
    cek "jamu siap (laravel)" no yes "jamu tidak menampilkan 'jamu: siap' dalam 40 detik"
    stop_jamu; akhir_grup "$log"; return
  fi
  cek "/ 200" "$(kode "$B/")" 200
  cek "/up 200" "$(kode "$B/up")" 200
  cek "/uji-db memuat pengguna" "$(has "$(cu "$B/uji-db")" pengguna)" yes
  s1=$(cu -c "$jar" -b "$jar" "$B/uji-sesi" | tr -d '[:space:]')
  s2=$(cu -c "$jar" -b "$jar" "$B/uji-sesi" | tr -d '[:space:]')
  cek "uji-sesi pertama" "$s1" '{"n":1}'
  cek "uji-sesi kedua" "$s2" '{"n":2}'
  cek "/tidak-ada 404" "$(kode "$B/tidak-ada")" 404
  cek "/.env 404" "$(kode "$B/.env")" 404
  # pekerja antrean dari jamu.toml mengerjakan tugas yang dikirim dari web
  cek "/uji-antre masuk antrean" "$(cu "$B/uji-antre")" masuk-antrean
  for _ in $(seq 1 40); do [ -s "$lar/storage/antre.txt" ] && break; sleep 0.5; done
  cek "pekerja antrean mengerjakan tugas" "$(cat "$lar/storage/antre.txt" 2>/dev/null)" selesai
  # Hanya public/index.php yang dijalankan: berkas .php lain di public tidak pernah jalan.
  printf '%s\n' '<?php echo 6 * 7, "-jalan";' > "$lar/public/lain.php"
  mkdir -p "$lar/public/sub"
  printf '%s\n' '<?php echo 6 * 7, "-jalan";' > "$lar/public/sub/index.php"
  local u
  for u in /lain.php /lain.php/x /LAIN.PHP /lain.php. /lain.php%20 /sub/ /sub/index.php; do
    bocor "$u tidak dijalankan" 42-jalan "$B$u"
    bocor "$u tidak tersaji sebagai sumber" '<?php' "$B$u"
  done
  cek "/lain.php 404 dari Laravel" "$(kode "$B/lain.php")" 404
  cek "/index.php/up 200" "$(kode "$B/index.php/up")" 200
  cek "/robots.txt 200" "$(kode "$B/robots.txt")" 200
  # penjadwal: tugas tiap menit jalan tanpa cron (paling lama menunggu satu pergantian menit)
  for _ in $(seq 1 150); do [ -s "$lar/storage/jadwal.txt" ] && break; sleep 0.5; done
  cek "penjadwal menjalankan tugas tiap menit" "$([ -s "$lar/storage/jadwal.txt" ] && echo yes || echo no)" yes
  stop_jamu
  akhir_grup "$log"
}

grup_wp() {
  echo "== wordpress =="
  if [ -z "${WP_DIR:-}" ]; then echo "lewati: wordpress (WP_DIR tidak diset)"; return; fi
  local wp="$work/wp" log="$work/wp.log" B=http://127.0.0.1:8103 jar="$work/wp.jar" loc rest r k
  cp -a "$WP_DIR" "$wp"
  if ! start_jamu "$wp" 8103 "$log"; then
    cek "jamu siap (wordpress)" no yes "jamu tidak menampilkan 'jamu: siap' dalam 40 detik"
    stop_jamu; akhir_grup "$log"; return
  fi
  # belum terpasang: redirect ke installer
  cek "/ 302" "$(kode "$B/")" 302
  loc=$(hdr "$B/" | grep '^location:' | tr -d ' ')
  cek "/ Location ke install.php" "$(akhiran "$loc" /wp-admin/install.php)" yes "location=$loc"
  # pemasangan
  k=$(cu -o "$work/install.html" -w '%{http_code}' "$B/wp-admin/install.php?step=2" \
    -d 'weblog_title=Uji&user_name=admin&admin_password=Rahasia-E2E-9911&admin_password2=Rahasia-E2E-9911&admin_email=admin@example.com&blog_public=0')
  cek "install step=2 200" "$k" 200
  cek "/ 200 setelah install" "$(kode "$B/")" 200
  cek "/ memuat 'Uji'" "$(has "$(cu "$B/")" Uji)" yes
  loc=$(hdr "$B/wp-admin/" | grep '^location:' | tr -d ' ')
  cek "/wp-admin/ 302 ke wp-login" "$(has "$loc" wp-login.php)" yes "location=$loc"
  # REST: pakai /wp-json/ bila berfungsi, jika tidak /?rest_route=/
  if [ "$(kode "$B/wp-json/")" = 200 ]; then rest="$B/wp-json/"; else rest="$B/?rest_route=/"; fi
  cek "REST API $rest 200" "$(kode "$rest")" 200
  cek "/tidak-ada/ 404 dari WordPress" "$(kode "$B/tidak-ada-sama-sekali/")" 404
  cek "/wp-login.php 200" "$(kode "$B/wp-login.php")" 200
  bocor "/wp-config.php tidak tersaji sebagai sumber" DB_PASSWORD "$B/wp-config.php"
  cek "aset core 200" "$(kode "$B/wp-includes/css/dist/block-library/style.min.css")" 200
  # PHP di folder unggahan harus diblokir
  mkdir -p "$wp/wp-content/uploads/2026"
  printf '%s\n' '<?php echo 1;' > "$wp/wp-content/uploads/2026/x.php"
  printf 'bukan php' > "$wp/wp-content/uploads/2026/gambar.png"
  cek "uploads x.php 403" "$(kode "$B/wp-content/uploads/2026/x.php")" 403
  cek "uploads x.php/y 403" "$(kode "$B/wp-content/uploads/2026/x.php/y")" 403
  cek "uploads x.php. 403" "$(kode --path-as-is "$B/wp-content/uploads/2026/x.php.")" 403
  cek "uploads x.php%20 403" "$(kode "$B/wp-content/uploads/2026/x.php%20")" 403
  cek "uploads gambar.png 200" "$(kode "$B/wp-content/uploads/2026/gambar.png")" 200
  # login
  cu -o /dev/null -c "$jar" -b "$jar" "$B/wp-login.php"
  r=$(cu -o /dev/null -w '%{http_code} %{redirect_url}' -c "$jar" -b "$jar" \
    -d 'log=admin&pwd=Rahasia-E2E-9911&wp-submit=Log+In&redirect_to=http://127.0.0.1:8103/wp-admin/&testcookie=1' \
    "$B/wp-login.php")
  cek "login 302" "${r%% *}" 302
  cek "login redirect ke /wp-admin/" "$(akhiran "${r#* }" /wp-admin/)" yes "redirect=${r#* }"
  cek "/wp-admin/ dengan login 200" "$(kode -c "$jar" -b "$jar" "$B/wp-admin/")" 200
  # Plugin cache sungguhan, bila ikut di situs uji: berkas yang ia buat disajikan Caddy.
  # Jawaban dari berkas dikenali dari Accept-Ranges, yang tidak pernah dikirim PHP.
  local sc="$wp/wp-content/plugins/wp-super-cache" f dari
  dari() { has "$(hdr "$@")" 'accept-ranges: bytes' | sed 's/yes/berkas/; s/no/php/'; }
  if [ -d "$sc" ]; then
    cek "sebelum plugin aktif: / dari PHP" "$(dari "$B/")" php
    r=$(cd "$wp" && "$JAMU" php "$here/wpsc.php" 127.0.0.1:8103 2>&1)
    cek "plugin cache diaktifkan" "$(has "$r" 'plugin aktif: ya, advanced-cache.php: ada')" yes "$r"
    cek "permintaan pertama dari PHP" "$(dari "$B/")" php
    f=""
    for _ in $(seq 1 10); do
      # Plugin juga menaruh index.html kosong di folder cache sebagai penutup daftar isi.
      f=$(find "$wp/wp-content/cache/supercache" -name index.html -size +0 2>/dev/null | head -1)
      [ -n "$f" ] && break
      cu -o /dev/null "$B/"; sleep 0.5
    done
    cek "plugin membuat berkas cache" "$([ -n "$f" ] && echo yes || echo no)" yes "$(find "$wp/wp-content/cache" 2>/dev/null | head -20)"
    cek "permintaan berikutnya dari berkas" "$(dari "$B/")" berkas
    cek "isinya sama dengan berkas plugin" "$(cu "$B/" | sha256sum | cut -d' ' -f1)" "$(sha256sum < "${f:-/dev/null}" | cut -d' ' -f1)"
    cek "dengan query: dari PHP" "$(dari "$B/?x=1")" php
    cek "pengunjung yang masuk log: dari PHP" "$(dari -c "$jar" -b "$jar" "$B/")" php
    cek "halaman admin tidak ikut di-cache" "$(kode -c "$jar" -b "$jar" "$B/wp-admin/")" 200
    # folder plugin dihapus tanpa dinonaktifkan: berkas cache tertinggal, tetapi tidak disajikan
    mv "$sc" "$sc.dihapus"
    cek "plugin dihapus paksa: dari PHP" "$(dari "$B/")" php
    cek "plugin dihapus paksa: situs tetap jalan" "$(kode "$B/")" 200
    mv "$sc.dihapus" "$sc"
  else
    echo "lewati: plugin cache (wp-super-cache tidak ada di situs uji)"
  fi
  stop_jamu
  akhir_grup "$log"
}

# Jalankan `jamu check`; hasilnya di $keluaran dan $rc.
periksa() { keluaran=$("$JAMU" check "$@" 2>&1); rc=$?; }

grup_check() {
  echo "== jamu check =="
  local d="$work/check" log="$work/check.log" keluaran rc
  mkdir -p "$d/polos" "$d/wp/wp-content/uploads/pribadi" "$d/wp/wp-content/uploads/gambar" \
    "$d/laravel/public" "$d/toml"
  printf '%s\n' '<?php echo "polos";' > "$d/polos/index.php"

  periksa "$d/polos"
  cek "situs bersih: kode keluar 0" "$rc" 0 "$keluaran"
  cek "situs bersih: tidak ada masalah" "$(has "$keluaran" 'Tidak ada masalah')" yes "$keluaran"
  cek "situs bersih: konfigurasi Caddy diuji" "$(has "$keluaran" 'konfigurasi Caddy diterima')" yes "$keluaran"
  periksa "$d/polos" --behind-proxy
  cek "mode proxy: konfigurasi diterima" "$rc" 0 "$keluaran"

  # check tidak boleh mengganggu situs yang sedang jalan
  start_jamu "$d/polos" 8108 "$log" || { cek "start polos" gagal ok; akhir_grup "$log"; return; }
  periksa "$d/polos"
  cek "saat situs jalan: check lulus" "$rc" 0 "$keluaran"
  cek "saat situs jalan: situs tetap menjawab" "$(cu http://127.0.0.1:8108/)" polos
  stop_jamu

  # jamu.toml
  printf '%s\n' '<?php' > "$d/toml/index.php"
  printf '%s\n' 'prot = "8.4"' > "$d/toml/jamu.toml"
  periksa "$d/toml"
  cek "jamu.toml salah ketik: kode keluar 1" "$rc" 1 "$keluaran"
  cek "jamu.toml salah ketik: kuncinya disebut" "$(has "$keluaran" prot)" yes "$keluaran"
  printf '%s\n' 'php = "7.0"' > "$d/toml/jamu.toml"
  periksa "$d/toml"
  cek "versi PHP tidak cocok: kode keluar 1" "$rc" 1 "$keluaran"
  cek "versi PHP tidak cocok: disebut" "$(has "$keluaran" 'meminta PHP 7.0')" yes "$keluaran"

  # .htaccess yang menutup folder
  printf '%s\n' '<?php' > "$d/wp/index.php"; printf '%s\n' '<?php' > "$d/wp/wp-load.php"
  printf '%s\n' '<?php' > "$d/wp/wp-config.php"
  printf '%s\n' '<FilesMatch "\.php$">' 'Deny from all' '</FilesMatch>' > "$d/wp/wp-content/uploads/gambar/.htaccess"
  printf '%s\n' 'RewriteEngine On' 'RewriteRule . /index.php [L]' > "$d/wp/.htaccess"
  periksa "$d/wp"
  cek "larangan per berkas: bukan masalah" "$rc" 0 "$keluaran"
  cek "larangan per berkas: dicatat" "$(has "$keluaran" 'wp-content/uploads/gambar/.htaccess baris 2')" yes "$keluaran"
  cek "aturan tulis ulang: tidak disebut" "$(printf '%s\n' "$keluaran" | grep -c '  \.htaccess')" 0 "$keluaran"
  printf '%s\n' '# dibuat plugin' '<IfModule mod_authz_core.c>' '  Require all denied' '</IfModule>' > "$d/wp/wp-content/uploads/pribadi/.htaccess"
  periksa "$d/wp"
  cek "folder ditutup .htaccess: kode keluar 1" "$rc" 1 "$keluaran"
  cek "folder ditutup .htaccess: foldernya disebut" "$(has "$keluaran" 'MASALAH  wp-content/uploads/pribadi/.htaccess')" yes "$keluaran"
  cek "folder ditutup .htaccess: jumlah masalah" "$(has "$keluaran" '1 masalah')" yes "$keluaran"

  # Laravel
  printf '%s\n' '<?php' > "$d/laravel/artisan"; printf '%s\n' '<?php' > "$d/laravel/public/index.php"
  periksa "$d/laravel"
  cek "laravel kosong: kode keluar 1" "$rc" 1 "$keluaran"
  cek "laravel kosong: vendor disebut" "$(has "$keluaran" 'folder vendor belum ada')" yes "$keluaran"
  cek "laravel kosong: APP_KEY disebut" "$(has "$keluaran" 'APP_KEY belum diisi')" yes "$keluaran"
  mkdir -p "$d/laravel/vendor"; printf '%s\n' '<?php' > "$d/laravel/vendor/autoload.php"
  printf '%s\n' 'APP_KEY=base64:abc' 'APP_DEBUG=true' > "$d/laravel/.env"
  JAMU_COMPOSER="$d/tidak-ada.phar" periksa "$d/laravel"
  cek "laravel dev dengan APP_DEBUG: bukan masalah" "$(has "$keluaran" APP_DEBUG)" no "$keluaran"
  JAMU_COMPOSER="$d/tidak-ada.phar" periksa "$d/laravel" --behind-proxy
  cek "laravel di server dengan APP_DEBUG: masalah" "$(has "$keluaran" 'MASALAH  APP_DEBUG')" yes "$keluaran"
  APP_DEBUG=false JAMU_COMPOSER="$d/tidak-ada.phar" periksa "$d/laravel" --behind-proxy
  cek "variabel lingkungan menang atas .env" "$(has "$keluaran" 'MASALAH  APP_DEBUG')" no "$keluaran"

  # Mode domain: nama yang belum ada di DNS adalah masalah; nama yang aneh ditolak sebelum
  # sempat ditulis ke konfigurasi.
  periksa "$d/polos" --domain tidak-ada.invalid --domain www.tidak-ada.invalid
  cek "domain tanpa DNS: kode keluar 1" "$rc" 1 "$keluaran"
  cek "domain tanpa DNS: tiap nama disebut" "$(printf '%s\n' "$keluaran" | grep -c 'belum punya rekaman DNS')" 2 "$keluaran"
  cek "domain tanpa DNS: konfigurasi dua nama diterima Caddy" "$(has "$keluaran" 'konfigurasi Caddy diterima')" yes "$keluaran"
  gagal_start "domain berisi kurung" 'tulis nama domain' serve "$d/polos" --domain 'situs.com {'
  gagal_start "domain berupa URL" 'tulis nama domain' serve "$d/polos" --domain https://situs.com
  gagal_start "email berisi baris baru" '--email' serve "$d/polos" --domain situs.com --email "$(printf 'a@b.c\n}')"

  # Caddy yang menolak konfigurasi: pesannya diteruskan
  if [ "$windows" = 0 ]; then
    printf '%s\n' '#!/bin/sh' 'echo "Error: adapting config: directive tidak dikenal" >&2' 'exit 1' > "$d/caddy-tolak"
    chmod +x "$d/caddy-tolak"
    JAMU_CADDY="$d/caddy-tolak" periksa "$d/polos"
    cek "Caddy menolak: kode keluar 1" "$rc" 1 "$keluaran"
    cek "Caddy menolak: pesannya diteruskan" "$(has "$keluaran" 'konfigurasi Caddy ditolak: Error: adapting config')" yes "$keluaran"
  fi
  akhir_grup "$log"
}

# Mode teruskan email: mail() diserahkan ke relay SMTP lewat `jamu sendmail` dan msmtp.
grup_mail() {
  echo "== email =="
  if [ "$windows" = 1 ] || ! command -v msmtp >/dev/null 2>&1; then
    echo "lewati: mode teruskan email (msmtp tidak ada)"; return
  fi
  local s1="$work/surat" s2="$work/surat-batas" log="$work/mail.log" kotak="$work/kotak.txt"
  local B=http://127.0.0.1:8141 smtp=8142 SP r n keluaran rc
  mkdir -p "$s1" "$s2"
  cat > "$s1/kirim.php" <<'EOF'
<?php
$kepala = isset($_GET['dari']) ? "From: Toko <{$_GET['dari']}>" : '';
// Bukan var_dump: dengan Xdebug terpasang keluarannya berupa HTML.
echo mail('tujuan@contoh.test', 'Uji jamu', "baris satu\n.\nbaris tiga", $kepala, $_GET['param'] ?? '') ? 'terkirim' : 'ditolak';
EOF
  cp "$s1/kirim.php" "$s2/kirim.php"
  printf '%s\n' '<?php echo ini_get("sendmail_path");' > "$s1/jalur.php"
  : > "$kotak"
  python3 "$here/smtp_palsu.py" "$smtp" "$kotak" & SP=$!
  for _ in $(seq 1 20); do (exec 3<>"/dev/tcp/127.0.0.1/$smtp") 2>/dev/null && break; sleep 0.25; done
  export JAMU_SMTP_HOST=127.0.0.1 JAMU_SMTP_PORT=$smtp JAMU_SMTP_TLS=off

  # Pengaturan yang salah ditolak saat menyalakan, bukan saat email pertama gagal.
  JAMU_SMTP_PORT=abc gagal_start "port relay bukan angka" JAMU_SMTP_PORT serve "$s1" --port 8141
  JAMU_SMTP_TLS=mungkin gagal_start "nilai TLS tidak dikenal" JAMU_SMTP_TLS serve "$s1" --port 8141
  JAMU_SMTP_USER=siti gagal_start "pengguna tanpa sandi" JAMU_SMTP_PASS serve "$s1" --port 8141
  JAMU_SMTP_USER=siti JAMU_SMTP_PASS=rahasia gagal_start "sandi tanpa TLS" 'tanpa enkripsi' serve "$s1" --port 8141
  JAMU_MAIL_FROM='bukan alamat' gagal_start "pengirim bawaan bukan alamat" JAMU_MAIL_FROM serve "$s1" --port 8141
  JAMU_MAIL_LIMIT=banyak gagal_start "batas bukan angka" JAMU_MAIL_LIMIT serve "$s1" --port 8141
  JAMU_MSMTP="$work/tidak-ada" gagal_start "msmtp tidak terpasang" msmtp serve "$s1" --port 8141
  gagal_start "sendmail diketik sendiri" 'dipanggil oleh PHP' sendmail -t

  # Tanpa relay, PHP tidak disentuh.
  JAMU_SMTP_HOST='' start_jamu "$s1" 8141 "$log" || { cek "start tanpa relay" gagal ok; kill $SP; akhir_grup "$log"; return; }
  cek "tanpa relay: sendmail_path bawaan PHP" "$(has "$(cu $B/jalur.php)" 'jamu')" no
  stop_jamu

  start_jamu "$s1" 8141 "$log" || { cek "start dengan relay" gagal ok; kill $SP; akhir_grup "$log"; return; }
  cek "relay disebut saat menyala" "$(has "$(cat "$log")" "mail() diteruskan ke 127.0.0.1:$smtp")" yes
  cek "mail() berhasil" "$(cu "$B/kirim.php?dari=toko@situs.test")" terkirim
  cek "pengirim amplop dari header From" "$(grep -c '^MAIL FROM:<toko@situs.test>' "$kotak")" 1 "$(cat "$kotak")"
  cek "penerima sampai ke relay" "$(grep -c '^RCPT TO:<tujuan@contoh.test>' "$kotak")" 1
  cek "judul sampai ke relay" "$(grep -c '^| Subject: Uji jamu' "$kotak")" 1
  # Baris berisi satu titik tidak boleh memotong email.
  cek "isi sesudah baris titik ikut terkirim" "$(grep -c '^| baris tiga' "$kotak")" 1 "$(cat "$kotak")"
  sleep 0.3
  cek "pengiriman tercatat di log" "$(grep -c 'email ke tujuan@contoh.test diteruskan' "$log")" 1 "$(tail -5 "$log")"

  cek "mail() dengan -f" "$(cu "$B/kirim.php?dari=toko@situs.test&param=-fpantul@situs.test")" terkirim
  cek "-f menjadi pengirim amplop" "$(grep -c '^MAIL FROM:<pantul@situs.test>' "$kotak")" 1
  # Parameter kelima mail() adalah celah lama: opsi selain -f tidak boleh sampai ke msmtp.
  r=$(cu -G "$B/kirim.php" --data-urlencode "dari=toko@situs.test" \
    --data-urlencode "param=-fa@situs.test -X$work/bocor.log -C$work/bocor.conf --host=10.0.0.1")
  cek "parameter liar: email tetap terkirim" "$r" terkirim
  cek "parameter liar: -f dipakai" "$(grep -c '^MAIL FROM:<a@situs.test>' "$kotak")" 1
  cek "parameter liar: tidak ada berkas tertulis" "$(ya test -e "$work/bocor.log")" no
  r=$(cu -G "$B/kirim.php" --data-urlencode "dari=toko@situs.test" --data-urlencode "param=-f-oQ/tmp")
  cek "pengirim -f yang aneh diabaikan" "$r" terkirim
  cek "pengirim -f yang aneh: memakai header From" "$(grep -c '^MAIL FROM:<toko@situs.test>' "$kotak")" 2

  n=$(grep -c '^SELESAI' "$kotak")
  cek "tanpa pengirim: mail() gagal" "$(cu "$B/kirim.php")" ditolak
  cek "tanpa pengirim: tidak ada yang dikirim" "$(grep -c '^SELESAI' "$kotak")" "$n"
  sleep 0.3
  cek "tanpa pengirim: sebabnya di log" "$(has "$(cat "$log")" 'tidak punya pengirim')" yes "$(tail -5 "$log")"
  stop_jamu

  # PHP di baris perintah (pekerja, penjadwal, jamu php) memakai jalur yang sama.
  r=$(cd "$s1" && "$JAMU" php -r 'echo mail("cli@contoh.test", "dari cli", "isi", "From: cli@situs.test") ? "terkirim" : "ditolak";' 2>&1)
  cek "jamu php: mail() berhasil" "$(has "$r" terkirim)" yes "$r"
  cek "jamu php: sampai ke relay" "$(grep -c '^RCPT TO:<cli@contoh.test>' "$kotak")" 1
  r=$(cd "$s1" && JAMU_SMTP_PORT=8143 "$JAMU" php -r 'echo mail("cli@contoh.test", "dari cli", "isi", "From: cli@situs.test") ? "terkirim" : "ditolak";' 2>&1)
  cek "relay mati: mail() gagal" "$(has "$r" ditolak)" yes "$r"
  cek "relay mati: sebabnya disebut" "$(has "$r" 'email ke cli@contoh.test gagal: cannot connect')" yes "$r"

  # jamu check ikut memeriksa relay.
  periksa "$s1"
  cek "check: konfigurasi PHP dengan relay diterima" "$rc" 0 "$keluaran"
  cek "check: relay menjawab" "$(has "$keluaran" "relay SMTP 127.0.0.1:$smtp menjawab")" yes "$keluaran"
  JAMU_SMTP_PORT=8143 periksa "$s1"
  cek "check: relay mati adalah masalah" "$rc" 1 "$keluaran"
  cek "check: relay mati disebut" "$(has "$keluaran" 'MASALAH  relay SMTP 127.0.0.1:8143 tidak bisa dihubungi')" yes "$keluaran"
  JAMU_SMTP_PORT=abc periksa "$s1"
  cek "check: pengaturan relay yang salah adalah masalah" "$(has "$keluaran" 'MASALAH  JAMU_SMTP_PORT')" yes "$keluaran"

  # Pengirim bawaan dan batas laju per jam.
  JAMU_MAIL_FROM=bawaan@situs.test JAMU_MAIL_LIMIT=2 start_jamu "$s2" 8141 "$log" \
    || { cek "start dengan batas" gagal ok; kill $SP; akhir_grup "$log"; return; }
  cek "pengirim bawaan: mail() berhasil" "$(cu "$B/kirim.php")" terkirim
  cek "pengirim bawaan dipakai" "$(grep -c '^MAIL FROM:<bawaan@situs.test>' "$kotak")" 1 "$(cat "$kotak")"
  cek "pengirim bawaan menjadi header From" "$(grep -c '^| From: bawaan@situs.test' "$kotak")" 1
  cek "email kedua masih boleh" "$(cu "$B/kirim.php?dari=toko@situs.test")" terkirim
  n=$(grep -c '^SELESAI' "$kotak")
  cek "email ketiga ditolak" "$(cu "$B/kirim.php?dari=toko@situs.test")" ditolak
  cek "email keempat ditolak" "$(cu "$B/kirim.php?dari=toko@situs.test")" ditolak
  cek "yang ditolak tidak sampai ke relay" "$(grep -c '^SELESAI' "$kotak")" "$n"
  sleep 0.3
  cek "batas tercapai: satu baris di log, bukan satu per email" "$(grep -c 'batas 2 email per jam' "$log")" 1 "$(tail -5 "$log")"
  stop_jamu
  kill $SP 2>/dev/null; wait $SP 2>/dev/null
  unset JAMU_SMTP_HOST JAMU_SMTP_PORT JAMU_SMTP_TLS
  akhir_grup "$log"
}

# GRUP="check toml" menjalankan sebagian saja; tanpa itu semuanya.
for g in ${GRUP:-php check proxy toml mail wp_aturan laravel wp}; do "grup_$g"; done

echo "$lulus lulus, $gagal gagal"
# Folder kerja hanya berguna untuk memeriksa kegagalan; bila semua lulus, dibuang.
if [ "$gagal" -eq 0 ]; then
  rm -rf "$work"
else
  echo "direktori kerja: $work"
  exit 1
fi
