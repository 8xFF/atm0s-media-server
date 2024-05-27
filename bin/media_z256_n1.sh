RUST_LOG=atm0s_sdn_network::features::socket=debug,info \
RUST_BACKTRACE=1 \
cargo run -- \
    --http-port 4001 \
    --node-id 257 \
    --sdn-port 11001 \
    --sdn-zone 256 \
    --seeds 256@/ip4/127.0.0.1/udp/11000 \
    media \
        --allow-private-ip \
        --enable-token-api
