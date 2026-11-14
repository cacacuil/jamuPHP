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
<?php $oc = function_exists('opcache_get_status') ? opcache_get_status(false) : null;
echo 'https=', $_SERVER['HTTPS'] ?? 'tidak', ' uid=', posix_geteuid(), ' opcache=', $oc && $oc['opcache_enabled'] ? 'on' : 'off', ' ext=', implode(',', array_map('strtolower', get_loaded_extensions()));
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

# Mode teruskan email: sendmail_path yang ditulis jamu harus sampai ke msmtp di dalam image.
# Relay sengaja tidak ada, jadi yang diharapkan adalah kegagalan dengan alasan dari msmtp.
surat=$(docker run --rm -w /tmp --entrypoint jamu \
  -e JAMU_SMTP_HOST=127.0.0.1 -e JAMU_SMTP_PORT=9 -e JAMU_SMTP_TLS=off "$image" \
  php -r 'echo mail("a@contoh.test", "uji", "isi", "From: b@contoh.test") ? "terkirim" : "ditolak";' 2>&1 || true)
[[ $surat == *"gagal: cannot connect"* && $surat == *ditolak ]] || fail "mode teruskan email: $surat"

# Jumlah pekerja mengikuti batas memori container (64 MB per pekerja), bukan hanya jumlah core.
harap=$(( $(nproc) * 2 )); [ "$harap" -gt 6 ] && harap=6; [ "$harap" -lt 4 ] && harap=4
kecil=$(docker run --rm --memory 384m -v "$site:/app:ro" --entrypoint sh "$image" -c 'timeout 3 jamu serve /app --behind-proxy 2>&1; true')
[[ $kecil == *"maksimal $harap pekerja (batas memori 384 MB)"* ]] || fail "pekerja tidak mengikuti batas memori: $kecil"

body=$(get "$url/")
[[ $body == https=tidak* ]] || fail "halaman utama: $body"
[[ $body == *" uid=0 "* ]] && fail "PHP berjalan sebagai root"
# Tanpa opcache setiap permintaan mengompilasi ulang semua berkas PHP.
[[ $body == *" opcache=on "* ]] || fail "opcache tidak aktif di php-fpm: ${body%% ext=*}"
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
