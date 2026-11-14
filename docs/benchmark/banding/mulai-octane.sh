#!/bin/sh
# Octane dan Caddy dalam satu container. Jumlah pekerja disamakan dengan jamu: dua per core.
# Pekerja diganti tiap 1000 permintaan, sama dengan berkas worker FrankenPHP milik Octane.
set -e
php artisan octane:start --server="$MESIN" --host=127.0.0.1 --port=8000 \
  --workers=$(( $(nproc) * 2 )) --max-requests=1000 --no-interaction &
exec caddy run --adapter caddyfile --config /banding/Caddyfile.octane
