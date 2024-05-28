RUST_LOG=atm0s_sdn_network::features::socket=debug,info \
RUST_BACKTRACE=1 \
cargo run -- \
    --http-port 4000 \
    --node-id 256 \
    --sdn-zone 256 \
    --sdn-port 11000 \
    --seeds 0@/ip4/127.0.0.1/udp/10000 \
    gateway \
        --lat 20 \
        --lon 30 \
        --geo-db "../maxminddb-data/GeoLite2-City.mmdb"
