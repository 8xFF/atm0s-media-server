# Single zone

Single zone is the simplest way to deploy a cluster. It is suitable for small scale deployment, testing and development.
When deploying a single zone cluster, all nodes are in the same zone. The cluster has the following nodes:

- Inner Gateway nodes
- Media server nodes (WebRTC, SIP or RTMP depending on your needs)

In this mode, the gateway will route requests to the best node based on the load, and some users in the same room may be routed to different media server nodes.

The architecture of a single zone cluster is as follows:

![Single zone](../../imgs/single-zone.excalidraw.png)

## Prerequisites

- [Install Docker](https://docs.docker.com/engine/install/)
- Prepare a secret:
- Prepare a zone prefix (it can be 0x000000, 24bit):
- Prepare a domain for gateway endpoint:
- Prepare a proxy to route traffic to gateway endpoint (optional):
- Prepare server index for each node (from 0 to 255):

## Deploy some gateway nodes

First pick index for your gateway nodes, for example 1 to 10 is for gateway nodes.

```bash
TODO:
```

## Deploy some media webrtc nodes

If you need WebRTC, Whip or Whep you need deploy some webrtc nodes, select index for your webrtc nodes, for example 100 to 255 is for webrtc nodes.

```bash
TODO:
```

## Deploy some media sip nodes

If you need SIP, you need deploy some sip nodes, select index for your sip nodes, for example 10 to 60 is for sip nodes.

```bash
TODO:
```

## Deploy some media rtmp nodes

If you need RTMP, you need deploy some rtmp nodes, select index for your rtmp nodes, for example 60 to 100 is for rtmp nodes.

```bash
TODO:
```

## Testing your cluster

Now let testing your cluster by some embded samples or sdk samples