RUST_LOG=info \
RUST_BACKTRACE=1 \
cargo run -- \
    --http-port 4002 \
    --node-id 258 \
    --enable-private-ip \
    --sdn-port 11002 \
    --sdn-zone 256 \
    --seeds 256@/ip4/127.0.0.1/udp/11000 \
    media \
        --enable-token-api
