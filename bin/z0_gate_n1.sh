RUST_LOG=info \
RUST_BACKTRACE=1 \
cargo run -- \
    --http-port 3000 \
    --enable-private-ip \
    --sdn-port 10001 \
    --sdn-zone-id 0 \
    --sdn-zone-node-id 1 \
    --seeds-from-node-api "http://localhost:8080/api/node/address" \
    --workers 2 \
    gateway \
        --lat 10 \
        --lon 20 \
        --max-memory 100 \
        --max-disk 100 \
        --geo-db "../maxminddb-data/GeoLite2-City.mmdb"
