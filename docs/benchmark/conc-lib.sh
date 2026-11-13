fpmc () 
{ 
    docker run -d --name fpm $CPU -v $SITE:/app -v $B/php.ini:/usr/local/etc/php/conf.d/bench.ini:ro -v $B/${1:-fpm-ondemand.conf}:/usr/local/etc/php-fpm.d/zzz-bench.conf:ro $M/library/php:8.5-fpm-alpine > /dev/null
}
c_nginx () 
{ 
    fpmc $1;
    docker run -d --name web $CPU --network container:fpm -v $SITE:/app:ro -v $B/nginx-c.conf:/etc/nginx/nginx.conf:ro $M/library/nginx:alpine > /dev/null
}
c_caddy () 
{ 
    fpmc $1;
    docker run -d --name web $CPU --network container:fpm -v $SITE:/app:ro -v $B/Caddyfile.fpm:/etc/caddy/Caddyfile:ro $M/library/caddy:2-alpine > /dev/null
}
c_ap_def () 
{ 
    fpmc $1;
    docker run -d --name web $CPU --network container:fpm -v $SITE:/app:ro -v $B/apache-fcgi.conf:/etc/apache2/sites-enabled/000-default.conf:ro -v $B/empty.conf:/etc/apache2/conf-enabled/other-vhosts-access-log.conf:ro $M/library/php:8.5-apache sh -c "$APCMD" > /dev/null
}
c_ap_tun () 
{ 
    fpmc $1;
    docker run -d --name web $CPU --network container:fpm -v $SITE:/app:ro -v $B/apache-fcgi.conf:/etc/apache2/sites-enabled/000-default.conf:ro -v $B/mpm_event_tuned.conf:/etc/apache2/mods-available/mpm_event.conf:ro -v $B/empty.conf:/etc/apache2/conf-enabled/other-vhosts-access-log.conf:ro $M/library/php:8.5-apache sh -c "$APCMD" > /dev/null
}
ip () 
{ 
    docker inspect -f '{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}' fpm
}
waitup () 
{ 
    for i in $(seq 1 40);
    do
        [ "$(curl -s -o /dev/null --noproxy '*' -w '%{http_code}' -m 5 http://$1/cached.html)" = "200" ] && return 0;
        sleep 0.5;
    done;
    echo TIDAK-SIAP
}
UL=""
APCMD='a2dismod -f -q php mpm_prefork; a2disconf -q docker-php; a2enmod -q mpm_event proxy proxy_fcgi; exec apache2-foreground'
