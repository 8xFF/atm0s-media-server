RUST_LOG=info \
RUST_BACKTRACE=1 \
cargo run -- \
    --http-port 4000 \
    --enable-private-ip \
    --sdn-zone-id 1 \
    --sdn-zone-node-id 1 \
    --sdn-port 11000 \
    --seeds-from-url "http://localhost:8080/api/cluster/seeds?zone_id=1&node_type=Gateway" \
    --workers 2 \
    gateway \
        --lat 20 \
        --lon 30 \
        --max-memory 100 \
        --max-disk 100 \
        --geo-db "../maxminddb-data/GeoLite2-City.mmdb"
