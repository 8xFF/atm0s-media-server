# Single zone

Single zone is the simplest way to deploy a cluster. It is suitable for small scale deployment, testing and development.
When deploying a single zone cluster, all nodes are in the same zone.

Limitations: maximum 256 nodes in the same zone.

The cluster has the following nodes:

- Gateway nodes
- Media server nodes (WebRTC, SIP or RTMP depending on your needs)

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
| Gateway   | [0; 10)      |
| SIP       | [10; 60)     |
| RTMP      | [60; 90)     |
| Connector | [90; 100)    |
| WebRTC    | [100 to 255] |

## Deploy some gateway nodes

First pick index for your gateway nodes, for example 1 to 10 is for gateway nodes.

Node1:

```bash
docker run -d --name atm0s-media-gateway-1 \
    --net=host ghcr.io/8xff/atm0s-media-gateway:master \
    --http-port=8080 \
    --sdn-port=10010 \
    --zone-index=1 \
    --secret=insecure \
    gateway
```

After node1 started it will print out the node address like `10@/ip4/192.168.1.10/udp/10010/ip4/192.168.1.10/tcp/10010`, you can use it as a seed node for other nodes.

```bash
docker run -d --name atm0s-media-gateway-1 \
    --net=host ghcr.io/8xff/atm0s-media-gateway:master \
    --http-port=8080 \
    --zone-index=2 \
    --secret=insecure \
    --seeds FIRST_GATEWAY_ADDR \
    gateway
```

## Deploy some media webrtc nodes

If you need WebRTC, Whip or Whep you need deploy some webrtc nodes, select index for your webrtc nodes, for example 100 to 255 is for webrtc nodes.

WebRTC 1:

```bash
docker run -d --name atm0s-media-gateway-1 \
    --net=host ghcr.io/8xff/atm0s-media-gateway:master \
    --zone-index=100 \
    --secret=insecure \
    --seeds FIRST_GATEWAY_ADDR \
    webrtc
```

WebRTC 2:

```bash
docker run -d --name atm0s-media-gateway-1 \
    --net=host ghcr.io/8xff/atm0s-media-gateway:master \
    --zone-index=101 \
    --secret=insecure \
    --seeds FIRST_GATEWAY_ADDR \
    webrtc
```

## Deploy some media sip nodes

If you need SIP, you need deploy some sip nodes, select index for your sip nodes, for example 10 to 60 is for sip nodes.

SIP 1:

```bash
docker run -d --name atm0s-media-gateway-1 \
    --net=host ghcr.io/8xff/atm0s-media-gateway:master \
    --zone-index=10 \
    --secret=insecure \
    --seeds FIRST_GATEWAY_ADDR \
    sip \
    --addr SERVER_IP:5060
```

SIP 1:

```bash
docker run -d --name atm0s-media-gateway-1 \
    --net=host ghcr.io/8xff/atm0s-media-gateway:master \
    --zone-index=11 \
    --secret=insecure \
    --seeds FIRST_GATEWAY_ADDR \
    sip \
    --addr SERVER_IP:5060
```

## Deploy some media rtmp nodes

If you need RTMP, you need deploy some rtmp nodes, select index for your rtmp nodes, for example 60 to 100 is for rtmp nodes.

Rtmp 1:

```bash
docker run -d --name atm0s-media-gateway-1 \
    --net=host ghcr.io/8xff/atm0s-media-gateway:master \
    --zone-index=60 \
    --secret=insecure \
    --seeds FIRST_GATEWAY_ADDR \
    sip \
    --addr SERVER_IP:5060
```

Rtmp 2:

```bash
docker run -d --name atm0s-media-gateway-1 \
    --net=host ghcr.io/8xff/atm0s-media-gateway:master \
    --zone-index=60 \
    --secret=insecure \
    --seeds FIRST_GATEWAY_ADDR \
    sip \
    --addr SERVER_IP:5060
```

## Deploy some connector nodes

If you need RTMP, you need deploy some rtmp nodes, select index for your rtmp nodes, for example 60 to 100 is for rtmp nodes.

Connector 1:

```bash
docker run -d --name atm0s-media-gateway-1 \
    --net=host ghcr.io/8xff/atm0s-media-gateway:master \
    --zone-index=60 \
    --secret=insecure \
    --seeds FIRST_GATEWAY_ADDR \
    connector \
    --mq-uri nats://NATS_IP:4222
```

## Monitoring

Each cluster nodes will expose a dashboard and prometheus metrics, you can use it to monitor your cluster.

Example bellow is gateway node dashboard: `gateway_url/dashboard/`

![Monitoring](../../imgs/demo-monitor.png)

## Testing your cluster

Some samples required access to microphone and camera permission, therefore it need to run with https if you want to test with some remote servers. We have 2 options for that:

- Running gateway under a reverse proxy like NGINX for providing https
- Start gateway with `--http-tls` for switching to self-signed https server.

Now let testing your cluster by some embded samples or sdk samples, more info at [Quick Start](../quick-start/README.md)
