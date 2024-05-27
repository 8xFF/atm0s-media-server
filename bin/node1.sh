RUST_LOG=info \
RUST_BACKTRACE=1 \
cargo run -- \
    --http-port 3001 \
    --node-id 1 \
    --sdn-port 10001 \
    --seeds 0@/ip4/127.0.0.1/udp/10000 \
    media \
        --allow-private-ip \
        --enable-token-api
