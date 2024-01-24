cargo run --package atm0s-media-server -- \
--node-id 31 \
--http-port 8031 \
--sdn-port 10031 \
--sdn-group group1 \
--seeds 11@/ip4/127.0.0.1/udp/10011/ip4/127.0.0.1/tcp/10011 \
sip --addr 127.0.0.1:5060