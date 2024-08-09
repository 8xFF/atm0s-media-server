RUST_LOG=info \
RUST_BACKTRACE=1 \
cargo run -- \
    --enable-private-ip \
    --sdn-zone-id 1 \
    --sdn-zone-node-id 3 \
    --seeds 257@/ip4/127.0.0.1/udp/11000 \
    --workers 2 \
    media \
        --webrtc-port-seed 11300 \
        --enable-token-api
