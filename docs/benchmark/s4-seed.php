<?php
// Mengisi situs uji S4 setelah WordPress dipasang: 20 tulisan berisi sama dan
// permalink /%postname%/, supaya situs MariaDB dan situs SQLite menyajikan halaman
// yang sama. Pemakaian: php s4-seed.php /app
$_SERVER['HTTP_HOST'] = 'bench.test:8080';
$_SERVER['REQUEST_URI'] = '/';
require $argv[1] . '/wp-load.php';

$kalimat = 'Jamu adalah minuman tradisional yang dibuat dari rempah dan akar. '
    . 'Setiap pagi penjualnya berkeliling membawa botol berisi kunyit asam, beras kencur, dan temulawak. ';
$isi = str_repeat("<!-- wp:paragraph -->\n<p>" . trim(str_repeat($kalimat, 3)) . "</p>\n<!-- /wp:paragraph -->\n\n", 4);

for ($i = 1; $i <= 20; $i++) {
    $id = wp_insert_post([
        'post_title' => "Tulisan uji $i",
        'post_content' => $isi,
        'post_status' => 'publish',
        'post_author' => 1,
        'post_date' => sprintf('2026-01-%02d 08:00:00', $i),
    ], true);
    if (is_wp_error($id)) {
        fwrite(STDERR, $id->get_error_message() . "\n");
        exit(1);
    }
}
update_option('permalink_structure', '/%postname%/');
flush_rewrite_rules(false);
echo 'tulisan terbit: ', wp_count_posts()->publish, ', tema: ', get_stylesheet(), "\n";
