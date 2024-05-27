RUST_LOG=info \
RUST_BACKTRACE=1 \
cargo run -- \
    --http-port 3000 \
    --node-id 0 \
    --sdn-port 10000 \
    gateway \
        --lat 10 \
        --lon 20
