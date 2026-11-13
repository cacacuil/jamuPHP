S=/tmp/claude-0/-home-user/85e0f8c6-f92c-5d4d-a19d-271fef72280e/scratchpad; B=$S/bench; SITE=$S/wp/site; M=mirror.gcr.io
CPU="--cpuset-cpus 0,1"
pss() { # jumlah PSS (MB) semua proses di container yang disebut
  local t=0 p k; for c in "$@"; do for p in $(docker top $c -eo pid 2>/dev/null | tail -n +2); do
    k=$(awk '/^Pss:/{print $2}' /proc/$p/smaps_rollup 2>/dev/null); t=$((t + ${k:-0})); done; done
  awk -v k=$t 'BEGIN{printf "%.0f", k/1024}'; }
nproc_c() { local n=0; for c in "$@"; do n=$((n + $(docker top $c -eo pid 2>/dev/null | tail -n +2 | wc -l))); done; echo $n; }
down() { docker rm -f web fpm >/dev/null 2>&1; }
ready() { for i in $(seq 1 40); do [ "$(curl -s -o /dev/null -w '%{http_code}' -m 5 http://127.0.0.1:8080/$1)" = "$2" ] && return 0; sleep 0.5; done; echo "TIDAK SIAP"; return 1; }
fpm() { docker run -d --name fpm $CPU -p 8080:80 -v $SITE:/app -v $B/php.ini:/usr/local/etc/php/conf.d/bench.ini:ro -v $B/$1:/usr/local/etc/php-fpm.d/zzz-bench.conf:ro $M/library/php:8.5-fpm-alpine >/dev/null; }
up_nginx()   { fpm fpm-static.conf;   docker run -d --name web $CPU --network container:fpm -v $SITE:/app:ro -v $B/nginx.conf:/etc/nginx/nginx.conf:ro $M/library/nginx:alpine >/dev/null; }
up_caddy()   { fpm fpm-static.conf;   docker run -d --name web $CPU --network container:fpm -v $SITE:/app:ro -v $B/Caddyfile.fpm:/etc/caddy/Caddyfile:ro $M/library/caddy:2-alpine >/dev/null; }
up_caddy_od(){ fpm fpm-ondemand.conf; docker run -d --name web $CPU --network container:fpm -v $SITE:/app:ro -v $B/Caddyfile.fpm:/etc/caddy/Caddyfile:ro $M/library/caddy:2-alpine >/dev/null; }
up_apache()  { docker run -d --name web $CPU -p 8080:80 -v $SITE:/app -v $B/php.ini:/usr/local/etc/php/conf.d/bench.ini:ro -v $B/apache-site.conf:/etc/apache2/sites-enabled/000-default.conf:ro -v $B/mpm_prefork.conf:/etc/apache2/mods-available/mpm_prefork.conf:ro -v $B/empty.conf:/etc/apache2/conf-enabled/other-vhosts-access-log.conf:ro $M/library/php:8.5-apache >/dev/null; }
up_franken() { docker run -d --name web $CPU -p 8080:80 -v $SITE:/app -v $B/php.ini:/usr/local/etc/php/conf.d/bench.ini:ro -v $B/Caddyfile.franken:/etc/frankenphp/Caddyfile:ro $M/dunglas/frankenphp:latest >/dev/null; }
