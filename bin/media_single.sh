RUST_LOG=atm0s_sdn_network=error,info \
RUST_BACKTRACE=1 \
cargo run -- \
    --sdn-zone-id 0 \
    --sdn-zone-node-id 1 \
    --workers 1 \
    --http-port 3000 \
    media \
	--enable-token-api \
	--disable-gateway-agent \
	--disable-connector-agent
