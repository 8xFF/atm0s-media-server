RUST_LOG=info \
RUST_BACKTRACE=1 \
cargo run -- \
    --http-port 3000 \
    --node-id 4 \
    --sdn-port 10004 \
    --sdn-zone 0 \
    --seeds 0@/ip4/127.0.0.1/udp/10000 \
    connector
