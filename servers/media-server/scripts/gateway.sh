cargo run --package atm0s-media-server -- \
--node-id 11 \
--http-port 8011 \
--sdn-port 10011 \
--sdn-zone zone1 \
gateway \
--lat 37.7749 \
--lng 122.4194 \
--geoip-db ../../../maxminddb-data/GeoLite2-City.mmdb
