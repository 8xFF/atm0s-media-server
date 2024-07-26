RUST_LOG=info \
RUST_BACKTRACE=1 \
cargo run -- \
    --http-port 4000 \
    --node-id 256 \
    --enable-private-ip \
    --sdn-zone 256 \
    --sdn-port 11000 \
    --seeds 0@/ip4/127.0.0.1/udp/10000 \
    --workers 2 \
    gateway \
        --lat 20 \
        --lon 30 \
        --max-memory 100 \
        --max-disk 100 \
        --geo-db "../maxminddb-data/GeoLite2-City.mmdb"
