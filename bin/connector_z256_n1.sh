RUST_LOG=info \
RUST_BACKTRACE=1 \
cargo run -- \
    --http-port 3000 \
    --node-id 259 \
    --sdn-port 11003 \
    --sdn-zone 256 \
    --seeds 256@/ip4/127.0.0.1/udp/11000 \
    connector
