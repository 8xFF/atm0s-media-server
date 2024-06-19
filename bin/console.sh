RUST_LOG=info \
RUST_BACKTRACE=1 \
cargo run -- \
    --http-port 8080 \
    --node-id 0 \
    --sdn-port 10000 \
    --sdn-zone 0 \
    console
