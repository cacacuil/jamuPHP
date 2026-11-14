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
