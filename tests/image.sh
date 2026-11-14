#!/usr/bin/env bash
# Uji asap image container. Pemakaian: tests/image.sh [IMAGE]
# Daftar extension yang wajib ada bisa diganti lewat variabel EXTENSIONS.
set -euo pipefail

image=${1:-jamuphp:8.4}
extensions=${EXTENSIONS:-bcmath curl dom exif fileinfo gd igbinary imagick intl mbstring mysqli openssl pdo_mysql pdo_pgsql pdo_sqlite redis sodium zip}
site=$(mktemp -d)
name=jamu-uji-$$
trap 'docker rm -f "$name" >/dev/null 2>&1 || true; rm -r "$site"' EXIT

mkdir "$site/public"
echo 'RAHASIA=1' > "$site/.env"
echo 'RAHASIA=1' > "$site/public/.env"
cat > "$site/public/index.php" <<'PHP'
<?php echo 'https=', $_SERVER['HTTPS'] ?? 'tidak', ' uid=', posix_geteuid(), ' ext=', implode(',', array_map('strtolower', get_loaded_extensions()));
PHP
chmod -R a+rX "$site"

fail() { echo "GAGAL: $*" >&2; docker logs "$name" 2>&1 | tail -20 >&2; exit 1; }
get() { curl -sS --noproxy '*' --max-time 10 "$@"; }

docker run -d --name "$name" -p 127.0.0.1::8080 -v "$site:/app:ro" "$image" >/dev/null
url=http://$(docker port "$name" 8080 | head -1)

for _ in $(seq 60); do
  [ "$(docker inspect "$name" --format '{{.State.Health.Status}}')" = healthy ] && break
  sleep 0.5
done
[ "$(docker inspect "$name" --format '{{.State.Health.Status}}')" = healthy ] || fail "tidak pernah sehat"

[[ $(docker exec "$name" jamu composer --version 2>&1) == Composer* ]] || fail "jamu composer tidak jalan"
[[ $(docker exec "$name" jamu php -r 'echo PHP_SAPI;' 2>&1) == cli ]] || fail "jamu php tidak jalan"
# Composer harus bisa bekerja sebagai www-data, yang tidak punya folder rumah yang bisa ditulis.
docker run --rm --entrypoint sh "$image" -c 'mkdir /tmp/p && cd /tmp/p \
  && jamu composer init --name=uji/uji --no-interaction >/dev/null 2>&1 \
  && jamu composer dump-autoload >/dev/null 2>&1 && test -f vendor/autoload.php' \
  || fail "jamu composer tidak bisa menulis vendor/ sebagai www-data"

body=$(get "$url/")
[[ $body == https=tidak* ]] || fail "halaman utama: $body"
[[ $body == *" uid=0 "* ]] && fail "PHP berjalan sebagai root"
for ext in $extensions; do
  [[ ",${body##*ext=}," == *",$ext,"* ]] || fail "extension $ext tidak ada"
done

[[ $(get -H 'X-Forwarded-Proto: https' "$url/") == https=on* ]] || fail "HTTPS dari proxy tidak dikenali"
[ "$(get -o /dev/null -w '%{http_code}' "$url/.env")" = 404 ] || fail ".env tersaji"
[ "$(get -o /dev/null -w '%{http_code}' "$url/tidak-ada.txt")" = 200 ] || fail "front controller tidak menangkap path lain"

start=$(date +%s)
docker stop "$name" >/dev/null
[ $(( $(date +%s) - start )) -lt 5 ] || fail "docker stop lebih dari 5 detik"
[ "$(docker inspect "$name" --format '{{.State.ExitCode}}')" = 0 ] || fail "keluar dengan kode bukan 0"

echo "lulus: $image"
