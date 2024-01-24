cargo run --package atm0s-media-server -- \
--node-id 11 \
--http-port 8011 \
--sdn-port 10011 \
--sdn-group group1 \
--seeds 1@/ip4/127.0.0.1/udp/10001/ip4/127.0.0.1/tcp/10001 \
gateway \
--mode inner \
--group local \
--lat 37.7749 \
--lng 122.4194
