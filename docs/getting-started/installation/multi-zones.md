# Multi zones

You can deploy a multi-zone cluster to scale up your cluster. Each zone is a single-zone cluster, and you can deploy many zones across the regions.

In a multi-zone setup, the zones are interconnected. To achieve this, all gateway nodes are interconnected and each request will be routed to the closest zone's gateway node.

![Multi zones](../../imgs/multi-zones-abstract.excalidraw.png)

The gateway nodes also take part in routing media data between zones in the fastest path possible; data will be relayed if the direct connection is bad.

Note that you can deploy multi connectors in some zones to handle room and peer events. However, you need to handle these events yourself to ensure data consistency.

## Prerequisites

- Choose a different zone id for each zone, it is 24bit unsigned number.
- Select a secret for all zones.

## Deploying each zone, same as a single-zone cluster

The deployment steps are the same as for a single-zone cluster with addition `--sdn-zone-id ZONE_ID` param. However, starting from second zone, you don't need to add console node, instead of that you can reuse single console node for all zones.
