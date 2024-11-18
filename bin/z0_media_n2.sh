RUST_LOG=info \
RUST_BACKTRACE=1 \
cargo run -- \
    --enable-private-ip \
    --sdn-zone-id 0 \
    --sdn-zone-node-id 2 \
    --seeds-from-node-api "http://localhost:3000/api/node/address" \
    --workers 2 \
    media
