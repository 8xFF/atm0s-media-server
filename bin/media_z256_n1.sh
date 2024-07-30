RUST_LOG=info \
RUST_BACKTRACE=1 \
cargo run -- \
    --http-port 4001 \
    --node-id 257 \
    --enable-private-ip \
    --sdn-port 11001 \
    --sdn-zone 256 \
    --seeds 256@/ip4/127.0.0.1/udp/11000 \
    --workers 2 \
    media \
        --enable-token-api
