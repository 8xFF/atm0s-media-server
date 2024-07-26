RUST_LOG=info \
RUST_BACKTRACE=1 \
cargo run -- \
    --http-port 3002 \
    --node-id 2 \
    --enable-private-ip \
    --sdn-port 10002 \
    --sdn-zone 0 \
    --seeds 1@/ip4/127.0.0.1/udp/10001 \
    --workers 2 \
    media \
        --enable-token-api
