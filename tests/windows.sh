#!/usr/bin/env bash
# Uji khusus Windows, dijalankan di Git Bash: hal-hal yang di Linux tidak mungkin salah.
# Pemakaian: JAMU=dist/jamu.exe PHP_DIR=dist/php bash tests/windows.sh
set -u

JAMU="${JAMU:-./target/release/jamu.exe}"
PHP_DIR="${PHP_DIR:-dist/php}"
work=$(cygpath -m "$(mktemp -d)")
B=http://127.0.0.1:8111
log="$work/jamu.log"
lulus=0; gagal=0

cu() { curl --noproxy '*' -s --max-time 120 "$@"; }
cek() { # NAMA DIDAPAT HARAPAN
  if [ "$2" = "$3" ]; then echo "ok     $1"; lulus=$((lulus+1)); else echo "GAGAL  $1: dapat [$2], harap [$3]"; gagal=$((gagal+1)); fi
}
catat() { echo "info   $*"; }
jumlah() { tasklist //FI "IMAGENAME eq $1" //NH | grep -c -i "$1"; }
memori() { # jumlah proses dan total memori (K) untuk satu nama
  tasklist //FI "IMAGENAME eq $1" //FO CSV //NH | grep -i "$1" | awk -F'","' '{gsub(/[^0-9]/,"",$5); n++; t+=$5} END{printf "%d proses, %d MB", n, t/1024}'
}
mulai() {
  local t0; t0=$(date +%s%N)
  "$JAMU" serve "$site" --port 8111 2> "$log" &
  JPID=$!
  for _ in $(seq 1 200); do
    if grep -q 'jamu: siap' "$log" 2>/dev/null; then catat "siap dalam $(( ($(date +%s%N) - t0) / 1000000 )) ms"; return 0; fi
    sleep 0.2
  done
  echo "GAGAL  jamu tidak siap dalam 40 detik"; gagal=$((gagal+1)); cat "$log"; return 1
}
berhenti() { kill "$JPID" 2>/dev/null; wait "$JPID" 2>/dev/null; }

# Situs berbentuk WordPress (ada wp-load.php), supaya aturan folder unggahan ikut aktif.
site="$work/Situs Uji é"
mkdir -p "$site/wp-content/uploads" "$site/.git"
echo '<?php // RAHASIA-SUMBER' > "$site/wp-load.php"
echo "<?php /* RAHASIA-SUMBER */ echo 'halo';" > "$site/index.php"
echo "<?php /* RAHASIA-SUMBER */ echo 'konfig';" > "$site/konfigurasi-rahasia.php"
echo 'KUNCI=RAHASIA-ENV' > "$site/.env"
echo 'RAHASIA-GIT' > "$site/.git/config"
echo "<?php /* RAHASIA-SUMBER */ echo 'DIEKSEKUSI';" > "$site/wp-content/uploads/x.php"
echo "<?php file_put_contents(__DIR__ . '/pid.log', getmypid() . \"\\n\", FILE_APPEND | LOCK_EX); echo 'ok';" > "$site/pid.php"
echo "<?php usleep(700000); echo 'selesai';" > "$site/lambat.php"
cat > "$site/info.php" <<'PHP'
<?php header('Content-Type: text/plain');
echo 'ekstensi: ', implode(' ', array_map('strtolower', get_loaded_extensions())), "\n";
$s = function_exists('opcache_get_status') ? opcache_get_status(false) : null;
echo 'opcache=', $s && $s['opcache_enabled'] ? 'on' : 'off', ' hits=', $s['opcache_statistics']['hits'] ?? '-', "\n";
echo 'sapi=', PHP_SAPI, ' versi=', PHP_VERSION, ' tmp=', sys_get_temp_dir(), ' ini=', php_ini_loaded_file(), "\n";
PHP
cat > "$site/https.php" <<'PHP'
<?php header('Content-Type: text/plain');
$u = 'https://example.com/';
$c = curl_init($u); curl_setopt_array($c, [CURLOPT_RETURNTRANSFER => true, CURLOPT_TIMEOUT => 20]);
echo 'curl=', curl_exec($c) !== false ? 'ok' : 'gagal: ' . curl_error($c), "\n";
$r = @file_get_contents($u);
echo 'stream=', $r !== false ? 'ok' : 'gagal: ' . (error_get_last()['message'] ?? '?'), "\n";
PHP

awal_caddy=$(jumlah caddy.exe); awal_php=$(jumlah php-cgi.exe)
mulai || exit 1

echo "== dasar =="
cek "/ dijalankan PHP" "$(cu "$B/")" halo
info=$(cu "$B/info.php"); echo "$info" | sed 's/^/info   /'
for e in curl exif fileinfo gd intl mbstring mysqli openssl pdo_mysql pdo_pgsql pdo_sqlite sodium zip; do
  cek "ekstensi $e" "$(echo "$info" | head -1 | grep -c -w "$e")" 1
done
cek "opcache aktif" "$(echo "$info" | grep -c 'opcache=on')" 1

echo "== sumber PHP dan dotfile tidak boleh bocor =="
# Nama pendek ala DOS (8.3): file `.env` bisa punya nama kedua seperti ENV~1.
pendek() { (cd "$site" && powershell -NoProfile -Command "(New-Object -ComObject Scripting.FileSystemObject).$1('$2').ShortName" | tr -d '\r'); }
env83=$(pendek GetFile .env); git83=$(pendek GetFolder .git); konf83=$(pendek GetFile konfigurasi-rahasia.php)
catat "nama pendek: .env=$env83 .git=$git83 konfigurasi-rahasia.php=$konf83"
aman() { # URL: isi jawaban tidak boleh memuat sumber atau rahasia, dan PHP di uploads tidak boleh jalan
  local kode isi
  kode=$(cu --path-as-is -o "$work/isi.txt" -w '%{http_code}' "$B$1")
  isi=$(cat "$work/isi.txt")
  case "$isi" in
    *RAHASIA*) echo "GAGAL  $1 ($kode): isi bocor: $(head -c 80 "$work/isi.txt")"; gagal=$((gagal+1));;
    *DIEKSEKUSI*) echo "GAGAL  $1 ($kode): PHP di folder unggahan dijalankan"; gagal=$((gagal+1));;
    *) echo "ok     $1 ($kode)"; lulus=$((lulus+1));;
  esac
}
for u in /INDEX.PHP /index.pHp /index.php. /index.php.. /index.php%20 '/index.php::$DATA' '/index.php::$data' \
  '/index.php:$DATA' '/index.php%3a%3a$DATA' '/wp-load.php::$DATA' "/$konf83" '/KONFIG~1.PHP' \
  /.env /.ENV /.env. /.env%20 '/.env::$DATA' "/$env83" '/ENV~1' \
  /.git/config /.GIT/config "/$git83/config" '/GIT~1/config' \
  /wp-content/uploads/x.php /wp-content/uploads/X.PHP /wp-content/uploads/x.php. /wp-content/uploads/x.php%20 \
  '/wp-content/uploads/x.php::$DATA' /wp-content/uploads/x.php/y /WP-CONTENT/UPLOADS/x.php; do
  aman "$u"
done

echo "== hanya loopback =="
ip=$(powershell -NoProfile -Command "(Get-NetIPAddress -AddressFamily IPv4 | Where-Object { \$_.IPAddress -notlike '127.*' -and \$_.IPAddress -notlike '169.254.*' } | Select-Object -First 1).IPAddress" | tr -d '\r')
if [ -n "$ip" ]; then
  curl --noproxy '*' -s --max-time 5 -o /dev/null "http://$ip:8111/"; rc=$?
  cek "tidak terbuka di $ip" "$([ "$rc" -ne 0 ] && echo tertutup || echo terbuka)" tertutup
else
  catat "tidak ada alamat selain loopback; dilewati"
fi

echo "== pekerja php-cgi =="
catat "php-cgi.exe: $(memori php-cgi.exe); caddy.exe: $(memori caddy.exe); jamu.exe: $(memori jamu.exe)"
t0=$(date +%s%N)
n=$(cu -Z --parallel-max 24 -o /dev/null -w '%{http_code}\n' "$B/lambat.php?i=[1-24]" | grep -c '^200$')
cek "24 permintaan lambat sekaligus" "$n" 24
catat "24 x 0,7 detik selesai dalam $(( ($(date +%s%N) - t0) / 1000000 )) ms"
# Tiap pekerja keluar setelah 500 permintaan. 6000 permintaan memaksa semuanya diganti berkali-kali.
t0=$(date +%s%N)
cu -Z --parallel-max 16 -o /dev/null -w '%{http_code}\n' "$B/pid.php?i=[1-6000]" | sort | uniq -c > "$work/kode.txt"
catat "6000 permintaan dalam $(( ($(date +%s%N) - t0) / 1000000 )) ms; kode: $(tr -s ' \n' ' ' < "$work/kode.txt")"
cek "6000 permintaan semuanya 200" "$(awk '$2==200{print $1}' "$work/kode.txt")" 6000
pid=$(sort -u "$site/pid.log" | wc -l | tr -d ' ')
catat "PID pekerja yang berbeda: $pid"
cek "pekerja didaur ulang (lebih dari 8 PID)" "$([ "$pid" -gt 8 ] && echo ya || echo tidak)" ya
cek "induk php-cgi tidak pernah mati" "$(grep -c 'php-cgi berhenti' "$log")" 0
cek "tidak ada galat opcache atau fatal di log" "$(grep -c -i 'reattach\|fatal' "$log")" 0
catat "sesudah beban: php-cgi.exe: $(memori php-cgi.exe); caddy.exe: $(memori caddy.exe)"
cu "$B/info.php" | grep opcache | sed 's/^/info   /'

echo "== HTTPS dari PHP =="
catat "tanpa daftar CA: $(cu "$B/https.php" | tr '\n' ' ')"

echo "== supervisor dimatikan paksa =="
wpid=$(tasklist //FI "IMAGENAME eq jamu.exe" //FO CSV //NH | head -1 | cut -d, -f2 | tr -d '"')
taskkill //F //PID "$wpid" > /dev/null; wait "$JPID" 2>/dev/null
for _ in $(seq 1 15); do
  [ "$(jumlah caddy.exe)" = "$awal_caddy" ] && [ "$(jumlah php-cgi.exe)" = "$awal_php" ] && break
  sleep 0.2
done
cek "caddy ikut mati" "$(jumlah caddy.exe)" "$awal_caddy"
cek "php-cgi ikut mati" "$(jumlah php-cgi.exe)" "$awal_php"

echo "== HTTPS dari PHP dengan daftar CA di extras/ssl/cacert.pem =="
mkdir -p "$PHP_DIR/extras/ssl"
if curl -fsSL --max-time 60 -o "$PHP_DIR/extras/ssl/cacert.pem" https://curl.se/ca/cacert.pem; then
  if mulai; then
    r=$(cu "$B/https.php"); echo "$r" | sed 's/^/info   /'
    cek "curl ke HTTPS" "$(echo "$r" | grep -c '^curl=ok')" 1
    cek "file_get_contents ke HTTPS" "$(echo "$r" | grep -c '^stream=ok')" 1
    berhenti
  fi
else
  catat "cacert.pem tidak bisa diunduh; dilewati"
fi

echo "--- log jamu terakhir ---"; tail -15 "$log"
echo "$lulus lulus, $gagal gagal"
[ "$gagal" -eq 0 ]
