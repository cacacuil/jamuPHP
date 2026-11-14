<?php
// Menyalakan atau mematikan WP Super Cache di situs uji S4 tanpa membuka wp-admin.
//
// on : plugin diaktifkan (hook aktivasinya menulis wp-cache-config.php,
//      advanced-cache.php, dan WP_CACHE di wp-config.php), lalu cache dinyalakan dengan
//      setelan yang disarankan plugin: mode "Simple" (halaman cache disajikan PHP; mode
//      "Expert" bergantung pada .htaccess) dan tanpa cache untuk pengunjung yang dikenal.
// off: plugin dinonaktifkan; hook-nya menghapus advanced-cache.php dan isi cache.
//
// Pemakaian: php s4-cache.php /app on|off
$_SERVER['HTTP_HOST'] = 'bench.test:8080';
$_SERVER['REQUEST_URI'] = '/';
require $argv[1] . '/wp-load.php';
// Hook plugin memakai fungsi wp-admin (get_home_path dan kawan-kawan).
require_once ABSPATH . 'wp-admin/includes/plugin.php';
require_once ABSPATH . 'wp-admin/includes/file.php';
require_once ABSPATH . 'wp-admin/includes/misc.php';

$plugin = 'wp-super-cache/wp-cache.php';
if (($argv[2] ?? '') === 'on') {
    $galat = activate_plugin($plugin);
    if (is_wp_error($galat)) {
        fwrite(STDERR, $galat->get_error_message() . "\n");
        exit(1);
    }
    wp_cache_enable();
    wp_super_cache_enable();
    wp_cache_setting('wp_cache_not_logged_in', 2);
} elseif (($argv[2] ?? '') === 'off') {
    deactivate_plugins($plugin);
} else {
    fwrite(STDERR, "pemakaian: php s4-cache.php DIR on|off\n");
    exit(2);
}
echo 'plugin aktif: ', is_plugin_active($plugin) ? 'ya' : 'tidak';
echo ', advanced-cache.php: ', is_file(WP_CONTENT_DIR . '/advanced-cache.php') ? 'ada' : 'tidak ada';
echo ', WP_CACHE di wp-config.php: ', preg_match('/^define\(\s*.WP_CACHE.,\s*true/m', file_get_contents(ABSPATH . 'wp-config.php')) ? 'ya' : 'tidak', "\n";
