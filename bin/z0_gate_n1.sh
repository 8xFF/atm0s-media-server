RUST_LOG=info \
RUST_BACKTRACE=1 \
cargo run -- \
    --http-port 3000 \
    --enable-private-ip \
    --sdn-port 10001 \
    --sdn-zone-id 0 \
    --sdn-zone-idx 1 \
    --seeds 0@/ip4/127.0.0.1/udp/10000 \
    --workers 2 \
    gateway \
        --lat 10 \
        --lon 20 \
        --max-memory 100 \
        --max-disk 100 \
        --geo-db "../maxminddb-data/GeoLite2-City.mmdb"
