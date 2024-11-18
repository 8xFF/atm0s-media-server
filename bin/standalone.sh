RUST_LOG=atm0s_sdn_network=error,info \
RUST_BACKTRACE=1 \
cargo run -- \
    --sdn-zone-node-id 1 \
    --workers 1 \
    standalone \
        --geo-db "../maxminddb-data/GeoLite2-City.mmdb" \
        --max-cpu 100 \
        --max-memory 100 \
        --max-disk 100
