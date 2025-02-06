# Cluster discovery

We device cluster into some parts: seeds and zones

## Seeds

Console nodes are act as some seeds node to other cluster can connect to and
discover each other.

Each some seconds, the console nodes will advertise a message with it address to
all other console nodes. With this information, other console nodes can connect
to each other.

## Zones

Each zone is a single-zone cluster, and we can deploy many zones across the
regions. Each zone must have at least one gateway node to connect to seeds and
to other zones's gateway nodes.

Other zone's node connect to all this zone's gateway nodes.

Each some seconds, the console nodes also advertise a message to all gateway's
nodes, which allow all gateway nodes to connect to all console nodes. Sam with
console nodes, each gateway node will advertise it address to all other gateway
in all all other zones with that, we can create a full connected network between
gateway nodes, allow discovery best path between all zones.

Gateway nodes also advertise it address to all same zone's media, connector
nodes.

# Summary

Depend on node type, it will advertise address to estiblish entire network.
