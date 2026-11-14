#!/usr/bin/env bash
# Pengukuran S4: WordPress di image jamuphp (Caddy + php-fpm ondemand) dengan MariaDB dan
# WP Super Cache, tiap beban tiga kali jalan. Lingkungannya disiapkan s4-setup.sh.
#
#   WORK=/tmp/s4 bash docs/benchmark/s4.sh
#
# Susunan: server (jamu, MariaDB, pembanding) di core 0-1, wrk di core 2-3, permintaan
# langsung ke IP container. Tiga pintu masuk ke php-fpm yang SAMA di container jamu:
#   J  jamu apa adanya                                  (s4web  :8080)
#   C  Caddyfile buatan jamu + aturan supercache        (s4caddy:8080)
#   N  nginx; :8080 semua lewat PHP, :8081 dengan aturan supercache (s4nginx)
# Hasil mentah: satu baris "S4|..." per jalan di $OUT.
set -uo pipefail
: "${WORK:?}"
HERE=$(cd "$(dirname "$0")" && pwd)
IMG=${IMG:-jamuphp:8.4}
NGINX=${NGINX:-nginx:alpine}
OUT=${OUT:-$HERE/s4-results.txt}
DUR=${DUR:-15}
RUNS=${RUNS:-3}
CPU="--cpuset-cpus 0,1"
H='Host: bench.test:8080'
# Yang dikirim peramban masa kini; Caddy memilih zstd, nginx gzip.
AE='Accept-Encoding: gzip, deflate, br, zstd'
# Cookie komentator: pengunjung yang dikenal, tidak pernah dilayani dari cache.
COOKIE='Cookie: comment_author_6243017cf04c3d6c2700c873a31c1a6e=Budi'
STATIC=/wp-includes/css/wp-embed-template.min.css

catat() { echo "S4|$*" | tee -a "$OUT"; }
ip() { docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' "$1"; }
cu() { curl -s --noproxy '*' -m 60 "$@"; }

mem() { # mem <label>: PSS per nama proses di container jamu, dan total MariaDB
  local rinci db
  rinci=$(for p in $(docker top s4web -eo pid | tail -n +2); do
    echo "$(cat /proc/$p/comm 2>/dev/null) $(awk '/^Pss:/{print $2}' /proc/$p/smaps_rollup 2>/dev/null)"
  done | awk 'NF==2{k[$1]+=$2; n[$1]++; t+=$2} END{printf "total=%.0fMB", t/1024; for (c in k) printf " %s=%.0fMB/%dp", c, k[c]/1024, n[c]}')
  db=$(for p in $(docker top s4db -eo pid | tail -n +2); do awk '/^Pss:/{print $2}' /proc/$p/smaps_rollup 2>/dev/null; done | awk '{t+=$1} END{printf "%.0f", t/1024}')
  catat "mem|$1|jamu: $rinci|mariadb=${db}MB"
}

down() { docker rm -f s4web s4nginx s4caddy >/dev/null 2>&1; docker volume rm s4state >/dev/null 2>&1; }

up() { # up <my|sq>
  down
  docker run -d --name s4web --network s4 $CPU -v "$WORK/site-$1:/app" -v "$HERE:/bench:ro" \
    -v s4state:/var/lib/jamu "$IMG" >/dev/null
  for _ in $(seq 1 60); do
    [ "$(docker inspect -f '{{.State.Health.Status}}' s4web)" = healthy ] && break
    sleep 1
  done
  local sock; sock=$(docker exec s4web sh -c 'ls /var/lib/jamu/jamuphp/*/php-fpm.sock')
  # Pembanding berjalan sebagai www-data (uid 33) supaya boleh membuka soket php-fpm.
  sed "s|@SOCK@|$sock|" "$HERE/s4-nginx.conf" > "$WORK/nginx.conf"
  docker run -d --name s4nginx --network s4 $CPU -u 33:33 -v "$WORK/site-$1:/app:ro" \
    -v s4state:/var/lib/jamu -v "$WORK/nginx.conf:/etc/nginx/nginx.conf:ro" "$NGINX" >/dev/null
  # Caddy kedua: biner dan Caddyfile yang sama dengan jamu, ditambah aturan supercache.
  docker exec s4web sh -c 'cat /var/lib/jamu/jamuphp/*/Caddyfile' \
    | awk -v f="$HERE/s4-supercache.caddy" '/^\tencode /{while ((getline l < f) > 0) print l} {print}' \
    | sed 's|storage file_system .*|storage file_system "/tmp/caddy"|' > "$WORK/Caddyfile.sc"
  docker run -d --name s4caddy --network s4 $CPU -u 33:33 -v "$WORK/site-$1:/app:ro" \
    -v s4state:/var/lib/jamu -v "$WORK/Caddyfile.sc:/cfg/Caddyfile:ro" --entrypoint caddy "$IMG" \
    run --config /cfg/Caddyfile --adapter caddyfile >/dev/null
  J=$(ip s4web); C=$(ip s4caddy); N=$(ip s4nginx)
  for u in "$J:8080" "$C:8080" "$N:8080" "$N:8081"; do
    for _ in $(seq 1 40); do
      [ "$(cu -o /dev/null -w '%{http_code}' -H "$H" "http://$u$STATIC")" = 200 ] && continue 2
      sleep 0.5
    done
    echo "TIDAK SIAP: $u"; exit 1
  done
}

cek() { # cek <url> [header...]: apa yang sebenarnya dijawab server untuk beban ini
  local url=$1; shift
  local hs=(-H "$H") h dekode=() meta
  for h in "$@"; do hs+=(-H "$h"); [[ $h == Accept-Encoding* ]] && dekode=(--compressed); done
  meta=$(cu "${dekode[@]}" -o "$WORK/cek.body" -w '%{http_code},%{size_download}B,enc=%header{content-encoding}' "${hs[@]}" "$url")
  echo "$meta,penanda_cache=$(grep -c 'WP-Super-Cache' "$WORK/cek.body")"
}

satu() { # satu <koneksi> <url> [header...] -> "rps=.. p50=.. p99=.. non2xx=.. galat=.."
  local c=$1 url=$2; shift 2
  local hs=(-H "$H") h out
  for h in "$@"; do hs+=(-H "$h"); done
  out=$(taskset -c 2,3 wrk -t2 -c"$c" -d"${DUR}s" --timeout 30s --latency "${hs[@]}" "$url" 2>&1)
  awk '/^Requests\/sec:/{rps=$2} /^ +50%/{p50=$2} /^ +99%/{p99=$2}
       /Non-2xx or 3xx/{bad=$NF} /Socket errors/{sub(/^ +Socket errors: /, ""); gsub(/ /, ""); err=$0}
       END{printf "rps=%s|p50=%s|p99=%s|non2xx=%s|galat=%s", rps, p50, p99, bad+0, err==""?"0":err}' <<<"$out"
}

bench() { # bench <label> <koneksi> <url> [header...]
  local label=$1 c=$2 url=$3 k r; shift 3
  k=$(cek "$url" "$@")
  taskset -c 2,3 wrk -t2 -c"$c" -d5s --timeout 30s -H "$H" "${@/#/-H}" "$url" >/dev/null 2>&1 # pemanasan
  for r in $(seq 1 "$RUNS"); do
    catat "$label|c=$c|jalan=$r|$(satu "$c" "$url" "$@")|cek=$k"
    sleep 2
  done
}

tenang() { # tunggu antrean PHP habis sesudah banjir
  for _ in $(seq 1 120); do
    [ "$(cu -o /dev/null -w '%{http_code}' -m 2 -H "$H" -H "$COOKIE" "http://$J:8080/")" = 200 ] && return
    sleep 1
  done
}

banjir() { # banjir <label> <host:port>: 300 koneksi ke halaman tanpa cache, lalu ukur halaman cache
  local label=$1 u=$2 r f
  for r in $(seq 1 "$RUNS"); do
    taskset -c 2,3 wrk -t2 -c300 -d"$((DUR + 5))s" --timeout 30s -H "$H" -H "$COOKIE" "http://$u/" > "$WORK/banjir.out" 2>&1 &
    sleep 4
    catat "$label|c=8 saat 300 koneksi menunggu PHP|jalan=$r|$(satu 8 "http://$u/")|cek=-"
    wait
    f=$(awk '/^Requests\/sec:/{rps=$2} /Socket errors/{sub(/^ +Socket errors: /, ""); gsub(/ /, ""); err=$0} /Non-2xx/{bad=$NF}
             END{printf "rps=%s|non2xx=%s|galat=%s", rps, bad+0, err==""?"0":err}' "$WORK/banjir.out")
    catat "$label|banjirnya sendiri (c=300, tanpa cache)|jalan=$r|$f"
    tenang; sleep 3
  done
}

cache() { docker exec s4web php /bench/s4-cache.php /app "$1" | sed 's/^/cache: /'; }

: > "$OUT"
catat "meta|$(date -u +%FT%TZ)|image=$IMG|$(docker run --rm --entrypoint php "$IMG" -r 'echo "php=", PHP_VERSION, " opcache=", ini_get("opcache.enable"), " validate_timestamps=", ini_get("opcache.validate_timestamps");')|$(docker run --rm --entrypoint caddy "$IMG" version | cut -d' ' -f1 | sed 's/^/caddy=/')|mariadb=$(docker exec s4db mariadb -h127.0.0.1 -uwp -pwp -N -e 'select version()')|$(docker run --rm --entrypoint nginx "$NGINX" -v 2>&1 | sed 's/nginx version: //')|$(wrk --version 2>&1 | head -1 | cut -d' ' -f1,2)|core=$(nproc)|durasi=${DUR}s x $RUNS"

# --- Tahap 1: tanpa plugin cache ---------------------------------------------------------
up my; cache off
mem "MariaDB, baru hidup"
bench "J jamu|dinamis, MariaDB" 8 "http://$J:8080/"
mem "MariaDB, sesudah beban dinamis"
sleep 20
mem "MariaDB, 20 detik diam"
bench "N nginx|dinamis, MariaDB" 8 "http://$N:8080/"
bench "J jamu|dinamis, MariaDB" 64 "http://$J:8080/"
bench "J jamu|statis 7 KB" 8 "http://$J:8080$STATIC"
bench "N nginx|statis 7 KB" 8 "http://$N:8080$STATIC"
bench "J jamu|statis 7 KB, peramban" 8 "http://$J:8080$STATIC" "$AE"
bench "N nginx|statis 7 KB, peramban" 8 "http://$N:8080$STATIC" "$AE"

up sq; cache off
bench "J jamu|dinamis, SQLite" 8 "http://$J:8080/"
mem "SQLite, sesudah beban dinamis"

# --- Tahap 2: WP Super Cache menyala -----------------------------------------------------
up my; cache on
cu -o /dev/null -H "$H" "http://$J:8080/"
ls -la "$WORK/site-my/wp-content/cache/supercache/bench.test/index.html" || { echo "berkas cache tidak terbentuk"; exit 1; }
bench "J jamu|cache, disajikan PHP" 8 "http://$J:8080/"
mem "MariaDB + cache, sesudah beban cache"
bench "N nginx|cache, disajikan PHP" 8 "http://$N:8080/"
bench "C jamu+aturan|cache, disajikan Caddy" 8 "http://$C:8080/"
bench "N nginx+aturan|cache, disajikan nginx" 8 "http://$N:8081/"
bench "J jamu|cache, disajikan PHP, peramban" 8 "http://$J:8080/" "$AE"
bench "C jamu+aturan|cache, disajikan Caddy, peramban" 8 "http://$C:8080/" "$AE"
bench "N nginx+aturan|cache, disajikan nginx, peramban" 8 "http://$N:8081/" "$AE"
bench "J jamu|cache, disajikan PHP" 64 "http://$J:8080/"
bench "C jamu+aturan|cache, disajikan Caddy" 64 "http://$C:8080/"
bench "J jamu|dinamis ber-cookie (cache menyala)" 8 "http://$J:8080/" "$COOKIE"
banjir "J jamu|cache, disajikan PHP" "$J:8080"
banjir "C jamu+aturan|cache, disajikan Caddy" "$C:8080"
banjir "N nginx+aturan|cache, disajikan nginx" "$N:8081"
mem "MariaDB + cache, akhir"

cache off
down
echo "selesai: $OUT"
