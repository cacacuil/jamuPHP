#!/usr/bin/env bash
# Menyiapkan situs uji untuk tests/e2e.sh: Laravel baru (SQLite) dan WordPress yang belum
# dipasang (SQLite lewat drop-in). Butuh `php` di PATH dan akses internet.
# Pemakaian: bash tests/sites.sh [FOLDER]   (bawaan: sites)
set -euo pipefail
sites=${1:-sites}
# Unduhan di dalam folder tujuan, dengan path relatif: aman juga untuk PHP di Git Bash.
unduh=$sites/.unduh
mkdir -p "$unduh"

curl -fsSL -o "$unduh"/composer.phar https://getcomposer.org/download/latest-stable/composer.phar
php "$unduh"/composer.phar create-project laravel/laravel "$sites"/laravel --no-interaction --prefer-dist --quiet
cat >> "$sites"/laravel/routes/web.php <<'EOF'
Route::get('/uji-db', fn () => ['pengguna' => \Illuminate\Support\Facades\DB::table('users')->count()]);
Route::get('/uji-sesi', function () { session(['n' => session('n', 0) + 1]); return ['n' => session('n')]; });
EOF
(cd "$sites"/laravel && php artisan --version && php artisan migrate --force)

curl -fsSL -o "$unduh"/wp.zip https://wordpress.org/latest.zip
unzip -q "$unduh"/wp.zip -d "$sites"
mv "$sites"/wordpress "$sites"/wp
curl -fsSL -o "$unduh"/sqlite.zip https://downloads.wordpress.org/plugin/sqlite-database-integration.zip
unzip -q "$unduh"/sqlite.zip -d "$sites"/wp/wp-content/plugins
cp "$sites"/wp/wp-content/plugins/sqlite-database-integration/db.copy "$sites"/wp/wp-content/db.php
cat > "$sites"/wp/wp-config.php <<'EOF'
<?php
define( 'DB_NAME', 'wp' );
define( 'DB_USER', 'wp' );
define( 'DB_PASSWORD', 'wp' );
define( 'DB_HOST', 'localhost' );
define( 'DB_CHARSET', 'utf8mb4' );
define( 'DB_COLLATE', '' );
define( 'AUTH_KEY', 'uji-1' );
define( 'SECURE_AUTH_KEY', 'uji-2' );
define( 'LOGGED_IN_KEY', 'uji-3' );
define( 'NONCE_KEY', 'uji-4' );
define( 'AUTH_SALT', 'uji-5' );
define( 'SECURE_AUTH_SALT', 'uji-6' );
define( 'LOGGED_IN_SALT', 'uji-7' );
define( 'NONCE_SALT', 'uji-8' );
$table_prefix = 'wp_';
define( 'WP_DEBUG', false );
define( 'DISABLE_WP_CRON', true );
define( 'WP_HTTP_BLOCK_EXTERNAL', true );
define( 'AUTOMATIC_UPDATER_DISABLED', true );
if ( ! defined( 'ABSPATH' ) ) {
	define( 'ABSPATH', __DIR__ . '/' );
}
require_once ABSPATH . 'wp-settings.php';
EOF
rm -f "$unduh"/composer.phar "$unduh"/wp.zip "$unduh"/sqlite.zip
rmdir "$unduh"
