#!/usr/bin/env bash
# Menyiapkan lingkungan ukur S4, sekali jalan: MariaDB dan dua salinan WordPress yang isinya
# sama, satu memakai MariaDB (site-my) dan satu SQLite (site-sq). Sesudah ini: s4.sh.
#
#   WORK=/tmp/s4 WP_SRC=/jalur/wordpress WPSC_SRC=/jalur/wp-super-cache \
#     bash docs/benchmark/s4-setup.sh
#
# WP_SRC  : WordPress yang belum dipasang, dengan drop-in SQLite di wp-content/db.php dan
#           wp-config.php yang DB_NAME/DB_USER/DB_PASSWORD-nya 'wp' dan DB_HOST 'localhost'.
# WPSC_SRC: sumber plugin WP Super Cache.
set -euo pipefail
: "${WORK:?}" "${WP_SRC:?}" "${WPSC_SRC:?}"
HERE=$(cd "$(dirname "$0")" && pwd)
IMG=${IMG:-jamuphp:8.4}
DBIMG=${DBIMG:-mariadb:11}
H='Host: bench.test:8080'

docker network inspect s4 >/dev/null 2>&1 || docker network create s4 >/dev/null
docker rm -f s4db s4web >/dev/null 2>&1 || true
# MariaDB memakai konfigurasi bawaan image dan berbagi dua core yang sama dengan server web,
# seperti VPS kecil yang menjalankan keduanya.
docker run -d --name s4db --network s4 --cpuset-cpus 0,1 \
  -e MARIADB_DATABASE=wp -e MARIADB_USER=wp -e MARIADB_PASSWORD=wp \
  -e MARIADB_RANDOM_ROOT_PASSWORD=1 "$DBIMG" >/dev/null
# Lewat TCP: server sementara saat inisialisasi tidak membuka jaringan.
for _ in $(seq 1 120); do
  docker exec s4db mariadb -h127.0.0.1 -uwp -pwp wp -e 'select 1' >/dev/null 2>&1 && break
  sleep 1
done
docker exec s4db mariadb -h127.0.0.1 -uwp -pwp wp -e 'select version()'

for s in my sq; do
  [ -e "$WORK/site-$s" ] && { echo "sudah ada: $WORK/site-$s, hapus dulu"; exit 1; }
  cp -a "$WP_SRC" "$WORK/site-$s"
  mkdir -p "$WORK/site-$s/wp-content/plugins/wp-super-cache"
  (cd "$WPSC_SRC" && cp -a ./*.php inc js partials plugins rest src styling languages \
    "$WORK/site-$s/wp-content/plugins/wp-super-cache/")
  if [ "$s" = my ]; then
    rm "$WORK/site-my/wp-content/db.php"
    sed -i "s/define( 'DB_HOST', 'localhost' );/define( 'DB_HOST', 's4db' );/" "$WORK/site-my/wp-config.php"
  fi
  chown -R 33:33 "$WORK/site-$s" # www-data di dalam image

  docker run -d --name s4web --network s4 --cpuset-cpus 0,1 -v "$WORK/site-$s:/app" \
    -v "$HERE:/bench:ro" "$IMG" >/dev/null
  for _ in $(seq 1 60); do
    [ "$(docker inspect -f '{{.State.Health.Status}}' s4web)" = healthy ] && break
    sleep 1
  done
  ip=$(docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' s4web)
  # Pemasangan lewat HTTP, sama seperti tests/e2e.sh. Sandi ini hanya untuk situs uji lokal.
  curl -sS --noproxy '*' -m 120 -o /dev/null -w "site-$s: install %{http_code}\n" -H "$H" \
    "http://$ip:8080/wp-admin/install.php?step=2" \
    -d 'weblog_title=Uji+S4&user_name=admin&admin_password=Rahasia-S4-2211&admin_password2=Rahasia-S4-2211&admin_email=admin@example.com&blog_public=0'
  docker exec s4web php /bench/s4-seed.php /app
  curl -sS --noproxy '*' -m 60 -o /dev/null -w "site-$s: beranda %{http_code}, %{size_download} byte\n" \
    -H "$H" "http://$ip:8080/"
  docker rm -f s4web >/dev/null
done
