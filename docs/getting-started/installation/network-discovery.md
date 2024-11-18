# Network Discovery

We have two ways to discover other nodes in the network:

- Manually specify the seeds
- Query the node API

## Manually specify the seeds

Each time we start the node, we can manually specify the seeds.

```
cargo run -- --sdn-zone-id 0 --sdn-zone-node-id 1 --seeds 1@/ip4/127.0.0.1/udp/10001 media
```

This way is simple and easy to understand, but it's not flexible and is inconvenient to manage.

## Query the node API

We can use the node API to get the addresses of other nodes and then start the node with those addresses.

```
cargo run -- --sdn-zone-id 0 --sdn-zone-node-id 1 --seeds-from-node-api "http://localhost:3000" media
```

This way is flexible and convenient to manage, and we can also use it to dynamically get the addresses of other nodes.
A common use case is when deploying with docker-compose or kubernetes - we only need to set up the loadbalancer to point to the HTTP API of nodes, then use the API to provide addresses to other nodes.

For example, we might have a loadbalancer config like this:

| Zone | Node Type | Address                          |
| ---- | --------- | -------------------------------- |
| 0    | Console   | http://console.atm0s.cloud       |
| 0    | Gateway   | http://gateway.zone0.atm0s.cloud |
| 1    | Gateway   | http://gateway.zone1.atm0s.cloud |
| 2    | Gateway   | http://gateway.zone2.atm0s.cloud |

Then we can start nodes with config like this:

| Zone | Node Type | Seeds From API                   |
| ---- | --------- | -------------------------------- |
| 0    | Gateway   | http://console.atm0s.cloud       |
| 0    | Media     | http://gateway.zone0.atm0s.cloud |
| 1    | Gateway   | http://console.atm0s.cloud       |
| 1    | Media     | http://gateway.zone1.atm0s.cloud |
| 2    | Gateway   | http://console.atm0s.cloud       |
| 2    | Media     | http://gateway.zone2.atm0s.cloud |

