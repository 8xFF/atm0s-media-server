cargo run --package atm0s-media-server -- \
--node-id 1 \
--http-port 8001 \
--sdn-port 10001 \
gateway \
--mode global \
--geoip-db ../../../maxminddb-data/GeoLite2-City.mmdb