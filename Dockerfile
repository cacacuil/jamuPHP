# syntax=docker/dockerfile:1
#
# Image dasar JamuPHP: jamu (supervisor) + Caddy + php-fpm dalam satu container.
# Dipakai di belakang proxy yang mengurus HTTPS (Traefik di Dokploy). Port: 8080.
#
#   FROM jamuphp:8.4
#   COPY --chown=www-data:www-data . /app
#
ARG PHP_VERSION=8.4

FROM rust:1-bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release --locked

FROM caddy:2 AS caddy
FROM composer:2 AS composer

FROM php:${PHP_VERSION}-fpm
# Satu set tetap yang menutup kebutuhan WordPress dan Laravel. Bawaan image resmi sudah
# berisi curl, dom, fileinfo, mbstring, openssl, pdo_sqlite, sodium, dan opcache.
ARG PHP_EXTENSIONS="bcmath exif gd igbinary imagick intl mysqli pdo_mysql pdo_pgsql redis zip"
COPY --from=mlocati/php-extension-installer /usr/bin/install-php-extensions /usr/local/bin/
RUN install-php-extensions ${PHP_EXTENSIONS} \
 && cp "$PHP_INI_DIR/php.ini-production" "$PHP_INI_DIR/php.ini"
COPY --from=caddy /usr/bin/caddy /usr/local/bin/caddy
# Untuk `RUN jamu composer install` di Dockerfile project, tanpa tahap build terpisah.
COPY --from=composer /usr/bin/composer /usr/local/bin/composer
COPY --from=build /src/target/release/jamu /usr/local/bin/jamu

# Menautkan image di GHCR ke repositori sumbernya.
LABEL org.opencontainers.image.source="https://github.com/cacacuil/jamuPHP" \
      org.opencontainers.image.description="JamuPHP: jamu + Caddy + php-fpm dalam satu container"

# Tidak ada yang berjalan sebagai root. Konfigurasi yang dibangkitkan jamu, soket php-fpm,
# dan data Caddy ada di /var/lib/jamu, di luar folder situs.
ENV XDG_STATE_HOME=/var/lib/jamu
RUN mkdir -p /app /var/lib/jamu && chown www-data:www-data /app /var/lib/jamu
USER www-data
WORKDIR /app
EXPOSE 8080
# Traefik baru mengirim lalu lintas setelah pemeriksaan ini lulus, jadi deploy tidak
# memutus pengunjung. Yang diuji: Caddy menerima koneksi dan php-fpm menjawab.
HEALTHCHECK --interval=10s --timeout=3s --start-period=30s --start-interval=1s --retries=3 \
  CMD ["jamu", "health"]
# jamu adalah PID 1: ia meneruskan SIGTERM dari `docker stop` dan memungut proses yatim.
# Image resmi PHP memasang STOPSIGNAL SIGQUIT (untuk php-fpm); tanpa baris ini `docker stop`
# menunggu 10 detik lalu membunuh paksa.
STOPSIGNAL SIGTERM
ENTRYPOINT ["jamu"]
CMD ["serve", "/app", "--behind-proxy"]
