cargo run --package atm0s-media-server -- \
--node-id 12 \
--http-port 8012 \
--sdn-port 10012 \
--sdn-zone zone2 \
--seeds 11@/ip4/127.0.0.1/udp/10011/ip4/127.0.0.1/tcp/10011 \
gateway \
--lat 47.7749 \
--lng 112.4194 \
--geoip-db ../../../maxminddb-data/GeoLite2-City.mmdb
