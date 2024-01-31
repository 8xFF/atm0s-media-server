# Cluster

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