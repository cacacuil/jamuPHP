#!/usr/bin/env bash
# Uji end-to-end binary jamu: PHP biasa, Laravel, WordPress.
# Pemakaian: JAMU=... LARAVEL_DIR=... WP_DIR=... bash tests/e2e.sh
set -u

JAMU="${JAMU:-./target/release/jamu}"
case "$(uname -s)" in MINGW*|MSYS*|CYGWIN*) windows=1;; *) windows=0;; esac
if [ "$windows" = 1 ]; then PHP_PROC="${PHP_PROC:-php-cgi.exe}"; else PHP_PROC="${PHP_PROC:-php-fpm}"; fi

work=$(mktemp -d "${TMPDIR:-/tmp}/jamu-e2e.XXXXXX")
# Di Git Bash, path gaya Windows (C:/...) dimengerti bash maupun program Windows asli.
[ "$windows" = 1 ] && work=$(cygpath -m "$work")
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
# ya/tidak untuk perintah; has/akhiran untuk teks.
ya() { if "$@"; then echo yes; else echo no; fi; }
has() { case "$1" in *"$2"*) echo yes;; *) echo no;; esac; }
akhiran() { case "$1" in *"$2") echo yes;; *) echo no;; esac; }

# Jalankan jamu di latar belakang, tunggu baris "jamu: siap" (maks 40 detik).
start_jamu() { # DIR PORT LOG
  "$JAMU" serve "$1" --port "$2" 2> "$3" &
  JPID=$!
  for _ in $(seq 1 80); do
    grep -q 'jamu: siap' "$3" 2>/dev/null && return 0
    sleep 0.5
  done
  return 1
}
stop_jamu() { kill "$JPID" 2>/dev/null; wait "$JPID" 2>/dev/null; }

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
  cat > "$htd/lambat.php" <<'EOF'
<?php usleep(700000); echo 'selesai';
EOF
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
  # 4. kompresi gzip
  cek "gaya.css gzip" "$(has "$(hdr -H 'Accept-Encoding: gzip' "$B/gaya.css")" 'content-encoding: gzip')" yes
  # 5-6. unggah berkas
  local r
  r=$(cu -o "$work/up.out" -w '%{http_code}' -F "berkas=@$work/besar-100.bin" "$B/upload.php")
  cek "upload 100 MB kode" "$r" 200
  cek "upload 100 MB isi" "$(cat "$work/up.out")" "100000000 $h"
  cek "upload 200 MB 413" "$(kode -F "berkas=@$work/besar-200.bin" "$B/upload.php")" 413
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
  if [ "$mati" = yes ]; then wait "$p2"; rc=$?; else kill "$p2" 2>/dev/null; wait "$p2" 2>/dev/null; rc=0; fi
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

grup_laravel() {
  echo "== laravel =="
  if [ -z "${LARAVEL_DIR:-}" ]; then echo "lewati: laravel (LARAVEL_DIR tidak diset)"; return; fi
  local log="$work/laravel.log" B=http://127.0.0.1:8102 jar="$work/laravel.jar" s1 s2
  if ! start_jamu "$LARAVEL_DIR" 8102 "$log"; then
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
  stop_jamu
  akhir_grup "$log"
}

grup_php
grup_laravel
grup_wp

echo "$lulus lulus, $gagal gagal"
echo "direktori kerja: $work"
[ "$gagal" -eq 0 ]
