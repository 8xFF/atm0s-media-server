# Auto generate node_id

Most of the time, we rarely deploy new zones, so the zone-id can be manually specified. However, for nodes inside a zone, we usually use cloud or docker for easy management. This leads to the problem of having to manually specify node_ids inside a zone, which is inconvenient and error-prone.

So we have implemented several mechanisms to auto-generate node_ids from machine information.

## Auto generate node_id from local_ip

The idea is simple: most cloud providers or bare-metal servers will assign a unique private IP to each machine, typically in the form of 10.10.10.x, 192.168.1.x, etc.

We can use the last octet of the IP as the node_id.

If the subnet is larger than /24, we still use the last 8 bits of the IP as the node_id, though this carries some risk of collision. In such cases, we recommend switching to a /24 subnet or using the NodeId pool.

Example:
```
cargo run -- --sdn-zone-id 0 --sdn-zone-node-id-from-ip-prefix "10.10.10" console
```

## NodeId pool

Status: in progress