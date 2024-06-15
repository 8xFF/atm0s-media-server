RUST_LOG=info \
RUST_BACKTRACE=1 \
cargo run -- \
    --http-port 3000 \
    --node-id 1 \
    --sdn-port 10001 \
    --sdn-zone 0 \
    --seeds 0@/ip4/127.0.0.1/udp/10000 \
    gateway \
        --lat 10 \
        --lon 20 \
        --max-memory 100 \
        --max-disk 100 \
        --geo-db "../maxminddb-data/GeoLite2-City.mmdb"
