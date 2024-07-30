RUST_LOG=info \
RUST_BACKTRACE=1 \
cargo run -- \
    --node-id 4 \
    --sdn-port 10004 \
    --sdn-zone 0 \
    --seeds 1@/ip4/127.0.0.1/udp/10001 \
    connector \
    --s3-uri "http://minioadmin:minioadmin@127.0.0.1:9000/record"
