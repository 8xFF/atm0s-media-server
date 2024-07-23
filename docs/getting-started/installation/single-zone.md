# Single zone

Single zone is the simplest way to deploy a cluster. It is suitable for small scale deployment, testing and development.
When deploying a single zone cluster, all nodes are in the same zone.

Limitations: maximum 256 nodes in the same zone.

The cluster has the following nodes:

- Console node: manage panel and act as a seed node for bootstrapping other nodes
- Gateway nodes: route signaling request into correct media node
- Media server nodes: handling media transport like WebRTC, RTP, RTMP ...
- Connector node: logging, hooks, record uri signer

In this mode, the gateway will route requests to the best node based on the load, and some users in the same room may be routed to different media server nodes.

The architecture of a single zone cluster is as follows:

![Single zone](../../imgs/single-zone.excalidraw.png)

## Prerequisites

- [Install Docker](https://docs.docker.com/engine/install/)
- Prepare a cluster secret.
- Prepare a proxy to route traffic to gateway endpoint (optional).
- Prepare a node index rules.

Example we have a node index rules like:

| Node type | Index range  |
| --------- | ------------ |
| Console   | [0; 10)      |
| Gateway   | [10; 20)     |
| Connector | [20; 30)     |
| Media     | [100 to 255] |

## Deploy a console nodes

Console node will expose at least 1 tcp port and 1 udp port

```
docker run -d --name main-console --net=host ghcr.io/8xff/atm0s-media-gateway:master \
    --secret secr3t \
    --node-id 0 \
    --sdn-port 10000 \
    --sdn-zone 0 \
    --http-port 8080 \
    --enable-private-ip \
    console
```

After node 0 started it will print out the node address like `0@/ip4/127.0.0.1/udp/10000`, you can use it as a seed node for other nodes.
The console node also have simple UI at `http://IP:8080`, you can login with above secret: `secr3t`

![alt text](console_screen.png)
![alt text](console_screen2.png)

## Deploy some gateway nodes

Gateway node will expose at least 1 tcp port and 1 udp port

```
docker run -d --name gateway1 --net=host ghcr.io/8xff/atm0s-media-gateway:master \
    --secret secr3t \
    --node-id 10 \
    --sdn-port 10010 \
    --sdn-zone 0 \
    --http-port 3000 \
    --enable-private-ip \
    --seeds 0@/ip4/127.0.0.1/udp/10000 \
    gateway
```

```
docker run -d --name gateway2 --net=host ghcr.io/8xff/atm0s-media-gateway:master \
    --secret secr3t \
    --node-id 11 \
    --sdn-port 10011 \
    --sdn-zone 0 \
    --http-port 3001 \
    --enable-private-ip \
    --seeds 0@/ip4/127.0.0.1/udp/10000 \
    gateway
```

After gateway nodes started, it will print out the node address like: `10@/ip4/127.0.0.1/udp/10010` and `11@/ip4/127.0.0.1/udp/10011`, the address is used as a seed address for media and connector nodes

After started gateway node also expose openapi ui for better integrating:

## Deploy some connector nodes

We need separated connector node which take care store logging data and response signed s3 uri for storing record file.
In default it will store data inside a sqlite file. For connecting to other database, you can provide db-uri like `protocol://username:password@host/database`, with protocol is: `sqlite`, `mysql`, `postgres`

```bash
docker run -d --name main-connector --net=host ghcr.io/8xff/atm0s-media-gateway:master \
    --secret secr3t \
    --node-id 20 \
    --sdn-port 10020 \
    --sdn-zone 0 \
    --enable-private-ip \
    --seeds 10@/ip4/127.0.0.1/udp/10010 \
    --seeds 11@/ip4/127.0.0.1/udp/10011 \
    connector \
        --s3-uri "http://minioadmin:minioadmin@127.0.0.1:9000/record"
```

## Deploy some media nodes

```bash
docker run -d --name media1 --net=host ghcr.io/8xff/atm0s-media-gateway:master \
    --secret secr3t \
    --node-id 100 \
    --sdn-port 10100 \
    --sdn-zone 0 \
    --enable-private-ip \
    --seeds 10@/ip4/127.0.0.1/udp/10010 \
    --seeds 11@/ip4/127.0.0.1/udp/10011 \
    media \
        --allow-private-ip
```

```bash
docker run -d --name media2 --net=host ghcr.io/8xff/atm0s-media-gateway:master \
    --secret secr3t \
    --node-id 101 \
    --sdn-port 10101 \
    --sdn-zone 0 \
    --enable-private-ip \
    --seeds 10@/ip4/127.0.0.1/udp/10010 \
    --seeds 11@/ip4/127.0.0.1/udp/10011 \
    media \
        --allow-private-ip
```

## Testing your cluster

Some samples required access to microphone and camera permission, therefore it need to run with https if you want to test with some remote servers. We have 2 options for that:

- Running gateway under a reverse proxy like NGINX for providing https
- Start gateway with `--http-tls` for switching to self-signed https server.

Now let testing your cluster by some embded samples or sdk samples, more info at [Quick Start](../quick-start/README.md)
