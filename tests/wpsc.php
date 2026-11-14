<?php
// Menyalakan WP Super Cache di situs uji tanpa membuka wp-admin, dengan setelan yang
// disarankan plugin: mode "Simple" dan tanpa cache untuk pengunjung yang dikenal.
// Hook aktivasinya menulis wp-cache-config.php, advanced-cache.php, dan WP_CACHE di
// wp-config.php.
//
// Pemakaian, dari folder WordPress: jamu php wpsc.php HOST[:PORT]
$_SERVER['HTTP_HOST'] = $argv[1];
$_SERVER['REQUEST_URI'] = '/';
require getcwd() . '/wp-load.php';
// Hook plugin memakai fungsi wp-admin (get_home_path dan kawan-kawan).
require_once ABSPATH . 'wp-admin/includes/plugin.php';
require_once ABSPATH . 'wp-admin/includes/file.php';
require_once ABSPATH . 'wp-admin/includes/misc.php';

$plugin = 'wp-super-cache/wp-cache.php';
$galat = activate_plugin($plugin);
if (is_wp_error($galat)) {
    fwrite(STDERR, $galat->get_error_message() . "\n");
    exit(1);
}
wp_cache_enable();
wp_super_cache_enable();
wp_cache_setting('wp_cache_not_logged_in', 2);
echo 'plugin aktif: ', is_plugin_active($plugin) ? 'ya' : 'tidak';
echo ', advanced-cache.php: ', is_file(WP_CONTENT_DIR . '/advanced-cache.php') ? 'ada' : 'tidak ada', "\n";
