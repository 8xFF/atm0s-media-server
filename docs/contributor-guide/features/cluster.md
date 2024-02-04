# Cluster RFC-0003-media-global-cluster

Atm0s Media Server supports cluster mode out of the box. All nodes in the cluster will automatically discover each other and route requests to the best node. 

We can have multiple gateway nodes and multiple media-server nodes in a cluster, which ensures high availability and scalability. We also don't need any persistent database for the cluster; all data will be stored in memory with the help of decentralized key-value. In case the node holding the data is down, the data will be automatically synced to other nodes in the cluster after a while. The logic behind this is similar to [Kademlia DHT](https://en.wikipedia.org/wiki/Kademlia), where the node for each key is selected and routed by the XOR operator.

The network topology is now fixed. In this configuration, all media servers are connected to the same zone gateways, and all gateways are interconnected. To automate the topology building process, each node will establish connections with other servers that have local tags matching the connect tags.

| Server | Local Tags | Connect Tags |
|--------|------------|--------------|
| Gateway | gateway, gateway-{zone-id} | gateway |
| Media Server | media-{protocol}-{zone-id} | gateway-{zone-id} |
| Connector | connetor-{zone-id} | gateway-{zone-id} |

## Single zone

![Single zone](../../imgs/single-zone.excalidraw.png)

## Multi zones

![Multi zones](../../imgs/multi-zones.excalidraw.png)