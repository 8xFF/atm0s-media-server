RUST_LOG=info \
RUST_BACKTRACE=1 \
cargo run -- \
    --sdn-zone-id 0 \
    --sdn-zone-node-id 4 \
    --seeds-from-node-api "http://localhost:3000" \
    connector \
        --s3-uri "http://minioadmin:minioadmin@127.0.0.1:9000/record"
