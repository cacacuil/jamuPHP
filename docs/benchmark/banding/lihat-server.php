<?php
// Diagnosis untuk pembanding: variabel server yang menentukan alamat menurut WordPress.
header('Content-Type: text/plain');
foreach (['REQUEST_URI', 'SCRIPT_NAME', 'PHP_SELF', 'PATH_INFO', 'ORIG_PATH_INFO', 'DOCUMENT_URI', 'SCRIPT_FILENAME',
    'HTTP_HOST', 'SERVER_NAME', 'SERVER_PORT', 'HTTPS', 'QUERY_STRING', 'SERVER_SOFTWARE'] as $k) {
    echo $k, '=', $_SERVER[$k] ?? '(tidak ada)', "\n";
}
