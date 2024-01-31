# Cluster

Atm0s Media Server supports cluster mode out of the box. All nodes in the cluster will automatically discover each other and route requests to the best node. 

We can have multiple gateway nodes and multiple media-server nodes in a cluster, which ensures high availability and scalability. We also don't need any persistent database for the cluster; all data will be stored in memory with the help of decentralized key-value. In case the node holding the data is down, the data will be automatically synced to other nodes in the cluster after a while. The logic behind this is similar to [Kademlia DHT](https://en.wikipedia.org/wiki/Kademlia), where the node for each key is selected and routed by the XOR operator.

We have two cluster modes: single zone and multi zones.

![Single zone](../../imgs/single-zone.excalidraw.png)

![Multi zones](../../imgs/multi-zones.excalidraw.png)