RUST_LOG=info \
RUST_BACKTRACE=1 \
cargo run -- \
    --http-port 8080 \
    --sdn-port 10000 \
    --sdn-zone-id 0 \
    --sdn-zone-idx 0 \
    --enable-private-ip \
    --workers 2 \
    console
