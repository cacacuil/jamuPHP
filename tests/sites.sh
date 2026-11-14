#!/usr/bin/env bash
# Menyiapkan situs uji untuk tests/e2e.sh: Laravel baru (SQLite) dan WordPress yang belum
# dipasang (SQLite lewat drop-in). Butuh akses internet.
# Pemakaian: [JAMU=path/ke/jamu] bash tests/sites.sh [FOLDER]   (bawaan: sites)
#
# Laravel dibuat lewat `jamu composer` dan `jamu php`: jalur yang sama dengan yang dipakai
# pengguna, tanpa php.ini atau PATH yang disiapkan tangan.
set -euo pipefail
sites=${1:-sites}
JAMU="${JAMU:-./target/release/jamu}"
JAMU="$(cd "$(dirname "$JAMU")" && pwd)/$(basename "$JAMU")"
# Unduhan di dalam folder tujuan, dengan path relatif: aman juga untuk PHP di Git Bash.
unduh=$sites/.unduh
mkdir -p "$unduh"

"$JAMU" composer create-project laravel/laravel "$sites"/laravel --no-interaction --prefer-dist --quiet
cat >> "$sites"/laravel/routes/web.php <<'EOF'
Route::get('/uji-db', fn () => ['pengguna' => \Illuminate\Support\Facades\DB::table('users')->count()]);
Route::get('/uji-sesi', function () { session(['n' => session('n', 0) + 1]); return ['n' => session('n')]; });
EOF
(cd "$sites"/laravel && "$JAMU" php artisan --version && "$JAMU" php artisan migrate --force)

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
rm -f "$unduh"/wp.zip "$unduh"/sqlite.zip
rmdir "$unduh"
