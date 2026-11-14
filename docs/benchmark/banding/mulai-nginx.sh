#!/bin/sh
# Jumlah pekerja PHP disamakan dengan jamu: dua per core.
set -e
sed "s|@PEKERJA@|$(( $(nproc) * 2 ))|" /banding/fpm.conf > /tmp/fpm.conf
sed "s|@ROOT@|${ROOT:-/app}|" /banding/nginx.conf > /tmp/nginx.conf
php-fpm -y /tmp/fpm.conf &
exec nginx -c /tmp/nginx.conf -g 'daemon off;'
