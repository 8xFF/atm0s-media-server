RUST_LOG=info \
RUST_BACKTRACE=1 \
cargo run -- \
    --enable-private-ip \
    --sdn-zone-id 0 \
    --sdn-zone-node-id 2 \
    --seeds 1@/ip4/127.0.0.1/udp/10001 \
    --workers 2 \
    media \
        --enable-token-api
