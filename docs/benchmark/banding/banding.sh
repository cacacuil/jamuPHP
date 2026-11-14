#!/usr/bin/env bash
# Membandingkan image jamuphp dengan FrankenPHP (mode klasik dan worker) dan nginx + php-fpm:
# mesin yang sama, PHP yang sama, aplikasi yang sama, log akses menyala di semuanya.
# Butuh Docker, wrk, curl, unzip, dan akses internet.
#
#   docker build -t jamuphp:8.4 .
#   bash docs/benchmark/banding/banding.sh [FOLDER_HASIL]
#
# Semua container memakai jaringan host di port 8080, satu per satu, supaya jaringan Docker
# tidak ikut terukur.
set -euo pipefail

B=$(cd "$(dirname "$0")" && pwd)
W=${1:-banding-hasil}
mkdir -p "$W"
W=$(cd "$W" && pwd)
JAMU=${JAMU_IMAGE:-jamuphp:8.4}
FRANKEN=${FRANKEN_IMAGE:-dunglas/frankenphp:php8.4}
LAMA=${LAMA:-8}     # detik per putaran wrk
ULANG=${ULANG:-3}   # putaran per titik ukur; yang dilaporkan mediannya
U=http://127.0.0.1:8080

ms() { date +%s%3N; }
kode() { curl -s -o /dev/null --noproxy '*' -m 3 -w '%{http_code}' "$1" || true; }
berhenti() { docker rm -f b-uji >/dev/null 2>&1 || true; }
gagal() { echo "GAGAL: $*" >&2; docker logs b-uji 2>&1 | tail -15 >&2 || true; exit 1; }

# Halaman yang diukur per aplikasi, dengan teks yang harus ada di jawabannya.
halaman() {
  case $1 in
    halo) printf '%s\n' '/index.php halo' '/statis.html statis' ;;
    laravel) printf '%s\n' '/up Application up' '/ Laravel' ;;
    wp) printf '%s\n' '/ Situs Banding' ;;
  esac
}

# ---------------------------------------------------------------- aplikasi uji

# Menjalankan perintah dari image jamuphp di folder aplikasi, sebagai pengguna host.
di_image() { # SUBFOLDER PERINTAH ARGUMEN...
  docker run --rm --user "$(id -u):$(id -g)" -e HOME=/tmp -e COMPOSER_HOME=/tmp/composer \
    -v "$W/apps:/w" -w "/w/$1" --entrypoint "$2" "$JAMU" "${@:3}"
}

siapkan() {
  local apps=$W/apps
  if [ -e "$apps" ]; then echo "aplikasi uji sudah ada di $apps, dipakai ulang"; return; fi
  mkdir -p "$apps/halo"

  echo '<?php echo "halo ", PHP_VERSION;' > "$apps/halo/index.php"
  cat > "$apps/halo/info.php" <<'PHP'
<?php $oc = function_exists('opcache_get_status') ? opcache_get_status(false) : null;
echo 'PHP ', PHP_VERSION, ' ', PHP_SAPI, PHP_ZTS ? ' ZTS' : ' NTS',
  ' opcache=', $oc && $oc['opcache_enabled'] ? 'on' : 'off',
  ' jit=', $oc && ($oc['jit']['on'] ?? false) ? 'on' : 'off';
PHP
  # Meniru extension yang crash: proses PHP yang sedang melayani permintaan ini mati
  # karena SIGSEGV (sinyal 11).
  echo '<?php posix_kill(posix_getpid(), 11); echo "masih hidup";' > "$apps/halo/mati.php"
  { echo '<!doctype html><title>statis</title>'; head -c 6960 /dev/zero | tr '\0' 'x'; } > "$apps/halo/statis.html"

  di_image . composer create-project laravel/laravel laravel --no-interaction --prefer-dist --no-progress
  di_image laravel composer require laravel/octane --no-interaction --no-progress
  # Production, dan sesi di cookie: tanpa itu yang terukur adalah kunci SQLite milik
  # aplikasi, bukan servernya.
  sed -i -e 's/^APP_ENV=.*/APP_ENV=production/' -e 's/^APP_DEBUG=.*/APP_DEBUG=false/' \
    -e 's/^SESSION_DRIVER=.*/SESSION_DRIVER=cookie/' -e 's/^CACHE_STORE=.*/CACHE_STORE=file/' \
    -e 's/^LOG_CHANNEL=.*/LOG_CHANNEL=stderr/' "$apps/laravel/.env"
  echo "aplikasi: $(di_image laravel php artisan --version)" | tee -a "$W/info.txt"

  curl -fsSL -o "$W/wp.zip" https://wordpress.org/latest.zip
  unzip -q "$W/wp.zip" -d "$apps"
  mv "$apps/wordpress" "$apps/wp"
  curl -fsSL -o "$W/sqlite.zip" https://downloads.wordpress.org/plugin/sqlite-database-integration.zip
  unzip -q "$W/sqlite.zip" -d "$apps/wp/wp-content/plugins"
  cp "$apps/wp/wp-content/plugins/sqlite-database-integration/db.copy" "$apps/wp/wp-content/db.php"
  cp "$B/wp-config.php" "$apps/wp/wp-config.php"
  # WordPress dipasang sekali lewat jamu; basis data SQLite-nya lalu ikut ke semua image.
  # Alamatnya harus sama dengan alamat saat diukur, kalau tidak WordPress mengalihkan.
  chmod -R a+rwX "$apps/wp"
  berhenti
  docker run -d --name b-uji --network host -v "$apps/wp:/app" "$JAMU" >/dev/null
  for _ in $(seq 200); do
    [ "$(kode "$U/wp-admin/install.php")" = 200 ] && break
    sleep 0.1
  done
  curl -fsS --noproxy '*' -m 120 -o /dev/null "$U/wp-admin/install.php?step=2" \
    --data-urlencode 'weblog_title=Situs Banding' --data-urlencode 'user_name=admin' \
    --data-urlencode 'admin_password=Banding-123-uji!' --data-urlencode 'admin_password2=Banding-123-uji!' \
    --data-urlencode 'admin_email=admin@contoh.test' --data-urlencode 'blog_public=0' \
    || gagal "WordPress tidak bisa dipasang"
  [[ $(curl -s --noproxy '*' -m 30 "$U/") == *"Situs Banding"* ]] || gagal "WordPress belum terpasang"
  echo "aplikasi: WordPress $(sed -n "s/^\$wp_version = '\(.*\)';/\1/p" "$apps/wp/wp-includes/version.php") (SQLite)" | tee -a "$W/info.txt"
  # Berkas yang ditulis WordPress (basis data) milik www-data; host harus bisa menyalinnya.
  docker exec -u 0 b-uji chmod -R a+rX /app/wp-content
  berhenti
}

rakit() {
  local app siap
  docker build -q -t b-franken --build-arg DASAR="$FRANKEN" -f "$B/Dockerfile.franken" "$B"
  docker build -q -t b-nginx --build-arg DASAR="$JAMU" -f "$B/Dockerfile.nginx" "$B"
  for app in halo laravel wp; do
    siap=true
    # Cache konfigurasi, rute, dan view seperti deploy production; berkas worker Octane
    # disalin dari paketnya (yang dikerjakan `artisan octane:install`).
    [ "$app" = laravel ] && siap='php artisan optimize && cp vendor/laravel/octane/src/Commands/stubs/frankenphp-worker.php public/'
    docker build -q -t "b-jamu-$app" --build-arg DASAR="$JAMU" --build-arg SIAPKAN="$siap" -f "$B/Dockerfile.app" "$W/apps/$app"
    docker build -q -t "b-nginx-$app" --build-arg DASAR=b-nginx --build-arg SIAPKAN="$siap" -f "$B/Dockerfile.app" "$W/apps/$app"
    docker build -q -t "b-franken-$app" --build-arg DASAR=b-franken --build-arg PEMILIK=root --build-arg SIAPKAN="$siap" -f "$B/Dockerfile.app" "$W/apps/$app"
  done
  {
    echo "server: jamu = $(docker run --rm --entrypoint caddy "$JAMU" version | cut -d' ' -f1) (Caddy) + php-fpm ondemand"
    echo "server: franken = $(docker run --rm b-franken frankenphp version | head -1)"
    echo "server: nginx = $(docker run --rm --entrypoint nginx b-nginx -v 2>&1) + php-fpm static"
    for app in halo laravel wp; do
      echo "image: $app jamu=$(ukuran "b-jamu-$app") franken=$(ukuran "b-franken-$app") nginx=$(ukuran "b-nginx-$app") MB"
    done
  } | tee -a "$W/info.txt"
}
ukuran() { docker image inspect -f '{{.Size}}' "$1" | awk '{ printf "%.0f", $1 / 1048576 }'; }

# ---------------------------------------------------------------- menjalankan dan mengukur

# Menyalakan satu server dan mencetak milidetik sampai jawaban 200 pertama.
mulai() { # SERVER APP [opsi docker run...]
  local server=$1 app=$2 root=/app path t0
  shift 2
  [ "$app" = laravel ] && root=/app/public
  read -r path _ <<< "$(halaman "$app")"
  berhenti
  t0=$(ms)
  case $server in
    jamu)
      docker run -d --name b-uji --network host "$@" "b-jamu-$app" ;;
    franken | franken-worker)
      docker run -d --name b-uji --network host -e ROOT="$root" -e MAX_REQUESTS=500 -v "$B:/banding:ro" "$@" \
        "b-franken-$app" frankenphp run --adapter caddyfile --config "/banding/Caddyfile.$server" ;;
    nginx)
      docker run -d --name b-uji --network host -e ROOT="$root" -v "$B:/banding:ro" "$@" "b-nginx-$app" ;;
  esac > /dev/null
  for _ in $(seq 600); do
    if [ "$(kode "$U$path")" = 200 ]; then echo $(($(ms) - t0)); return; fi
    sleep 0.05
  done
  curl -s --noproxy '*' -m 10 -o /dev/null -D - "$U$path" | head -15 >&2 || true
  gagal "$server/$app tidak pernah menjawab 200 di $path"
}

periksa() { # SERVER APP: tiap halaman harus berisi teks yang diharapkan
  local path teks
  while read -r path teks; do
    [[ $(curl -s --noproxy '*' -m 30 "$U$path") == *"$teks"* ]] || gagal "$1/$2: $path tidak berisi '$teks'"
  done <<< "$(halaman "$2")"
}

# Memori container menurut Docker (cgroup, tanpa cache berkas), dalam MB.
memori() {
  docker stats --no-stream --format '{{.MemUsage}}' b-uji | awk '{
    v = $1
    if (v ~ /GiB/) printf "%.0f", v * 1024
    else if (v ~ /KiB/) printf "%.0f", v / 1024
    else printf "%.0f", v + 0
  }'
}

# Satu putaran wrk. Mencetak: req/s, p50 ms, p99 ms, jumlah galat, jumlah permintaan.
# KONEKSI berakhiran z: permintaan membawa Accept-Encoding seperti browser, jadi jawabannya
# dimampatkan. wrk sendiri tidak pernah memintanya.
wrk_sekali() { # KONEKSI URL [DETIK]
  local kepala=()
  case "$1" in *z) kepala=(-H 'Accept-Encoding: gzip, zstd') ;; esac
  wrk -t4 -c"${1%z}" -d"${3:-$LAMA}s" --latency "${kepala[@]}" "$2" 2>&1 | awk '
    function ms(v) {
      if (v ~ /us$/) return v / 1000
      if (v ~ /ms$/) return v + 0
      if (v ~ /m$/) return v * 60000
      return v * 1000
    }
    $1 == "50%" { p50 = ms($2) }
    $1 == "99%" { p99 = ms($2) }
    /^Requests\/sec:/ { rps = $2 }
    / requests in / { total = $1 }
    /Non-2xx or 3xx responses:/ { galat += $NF }
    /Socket errors:/ { gsub(",", ""); galat += $4 + $6 + $8 + $10 }
    END { printf "%.1f %.2f %.2f %d %d\n", rps, p50, p99, galat, total }'
}

ukur() { # SERVER APP
  local server=$1 app=$2 siap diam path c i mem baris
  siap=$(mulai "$server" "$app")
  periksa "$server" "$app"
  if [ "$app" = halo ]; then
    echo "php: $server = $(curl -s --noproxy '*' -m 5 "$U/info.php")" | tee -a "$W/info.txt"
    tail -n 1 "$W/info.txt" | grep -q 'opcache=on' || gagal "opcache tidak aktif di $server"
  fi
  sleep 2
  diam=$(memori)
  echo "$server $app $siap $diam" >> "$W/mulai.txt"
  while read -r path _; do
    wrk -t4 -c8 -d3s "$U$path" > /dev/null 2>&1 || true # pemanasan
    for c in 8 64 64z; do
      : > "$W/putaran.txt"
      mem=-
      for i in $(seq "$ULANG"); do
        if [ "$c" = 64 ] && [ "$i" = "$ULANG" ]; then
          wrk_sekali "$c" "$U$path" >> "$W/putaran.txt" &
          sleep $((LAMA / 2))
          mem=$(memori)
          wait
        else
          wrk_sekali "$c" "$U$path" >> "$W/putaran.txt"
        fi
      done
      baris=$(sort -n "$W/putaran.txt" | awk -v n="$ULANG" '
        { galat += $4 } NR == int((n + 1) / 2) { median = $1 " " $2 " " $3 } END { print median, galat }')
      echo "$server $app $path $c $baris $mem" | tee -a "$W/hasil.txt"
    done
  done <<< "$(halaman "$app")"
  berhenti
}

# Satu permintaan mematikan proses PHP yang melayaninya, selagi situs dibebani 16 koneksi.
# Container dijalankan dengan --restart always, seperti di production.
jatuh() { # SERVER
  local server=$1 akhir t k wrk putus
  mulai "$server" halo --restart always > /dev/null
  akhir=$(($(ms) + 13000))
  (while t=$(ms) && [ "$t" -lt "$akhir" ]; do
    echo "$t $(kode "$U/index.php")"
    sleep 0.05
  done) > "$W/denyut-$server.txt" &
  wrk_sekali 16 "$U/index.php" 12 > "$W/jatuh-wrk.txt" &
  sleep 4
  k=$(kode "$U/mati.php")
  wait
  wrk=$(cat "$W/jatuh-wrk.txt")
  # Lama putus: dari denyut gagal pertama sampai denyut berhasil berikutnya.
  putus=$(awk '
    $2 != 200 { n++; if (!a) a = $1 }
    $2 == 200 && a && !b { b = $1 }
    END { print (a ? (b ? b - a : "tidak-pulih") : 0), n + 0 }' "$W/denyut-$server.txt")
  echo "$server $k $wrk $putus $(docker inspect -f '{{.RestartCount}}' b-uji)" | tee -a "$W/jatuh.txt"
  berhenti
}

ringkas() {
  {
    echo "## Banding $(date -u +%F): $(nproc) core, $(awk '/MemTotal/ { printf "%.0f", $2 / 1048576 }' /proc/meminfo) GB, wrk $LAMA detik x $ULANG (median)"
    echo
    echo '```'
    cat "$W/info.txt"
    echo '```'
    echo
    echo 'Koneksi `64z`: 64 koneksi yang meminta jawaban dimampatkan, seperti browser.'
    echo
    echo '| beban | koneksi | server | req/s | p50 ms | p99 ms | galat | memori MB |'
    echo '|---|---|---|---|---|---|---|---|'
    sort -s -k2,2 -k3,3 -k4,4n "$W/hasil.txt" \
      | awk '{ printf "| %s %s | %s | %s | %s | %s | %s | %s | %s |\n", $2, $3, $4, $1, $5, $6, $7, $8, $9 }'
    echo
    echo '| aplikasi | server | siap (ms) | memori diam MB |'
    echo '|---|---|---|---|'
    sort -s -k2,2 "$W/mulai.txt" | awk '{ printf "| %s | %s | %s | %s |\n", $2, $1, $3, $4 }'
    echo
    echo '| server | jawaban mati.php | req/s | galat | permintaan | putus (ms) | denyut gagal | container diulang |'
    echo '|---|---|---|---|---|---|---|---|'
    awk '{ printf "| %s | %s | %s | %s | %s | %s | %s | %s |\n", $1, $2, $3, $6, $7, $8, $9, $10 }' "$W/jatuh.txt"
  } | tee "$W/ringkasan.md"
  if [ -n "${GITHUB_STEP_SUMMARY:-}" ]; then cat "$W/ringkasan.md" >> "$GITHUB_STEP_SUMMARY"; fi
}

utama() {
  local app server
  trap berhenti EXIT
  : > "$W/hasil.txt"
  : > "$W/mulai.txt"
  : > "$W/jatuh.txt"
  : > "$W/info.txt"
  siapkan
  rakit
  # Semua pasangan dicoba dulu sebentar: salah konfigurasi ketahuan sebelum pengukuran
  # yang lama dimulai.
  for app in halo laravel wp; do
    for server in jamu franken nginx; do
      mulai "$server" "$app" > /dev/null
      periksa "$server" "$app"
    done
  done
  mulai franken-worker laravel > /dev/null
  periksa franken-worker laravel
  # Server yang dibandingkan pada beban yang sama diukur berurutan, supaya naik-turunnya
  # mesin CI tidak memihak salah satu.
  for app in halo laravel wp; do
    for server in jamu franken nginx; do ukur "$server" "$app"; done
    if [ "$app" = laravel ]; then ukur franken-worker laravel; fi
  done
  for server in jamu franken nginx; do jatuh "$server"; done
  ringkas
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then utama; fi
