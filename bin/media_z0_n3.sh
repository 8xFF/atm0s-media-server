RUST_LOG=info \
RUST_BACKTRACE=1 \
cargo run -- \
    --http-port 3003 \
    --node-id 3 \
    --sdn-port 10003 \
    --sdn-zone 0 \
    --seeds 1@/ip4/127.0.0.1/udp/10001 \
    media \
        --allow-private-ip \
        --enable-token-api
