RUST_LOG=info \
RUST_BACKTRACE=1 \
cargo run -- \
    --enable-private-ip \
    --sdn-zone-id 0 \
    --sdn-zone-node-id 3 \
    --seeds-from-node-api "http://localhost:3000" \
    --workers 2 \
    media
