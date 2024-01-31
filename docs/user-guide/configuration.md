# Configuration

In this document, we will describe how to configure atm0s-media-server.

## Network and general configuration

```bash
Usage: atm0s-media-server [OPTIONS] <COMMAND>

Commands:
  token-generate  Decentralized media-server with WebRTC/RTMP/Whip/Whep support
  gateway         Decentralized media-server with WebRTC/RTMP/Whip/Whep support
  webrtc          Decentralized media-server with WebRTC/RTMP/Whip/Whep support
  rtmp            Decentralized media-server with WebRTC/RTMP/Whip/Whep support
  sip             Decentralized media-server with WebRTC/RTMP/Whip/Whep support
  connector       Decentralized media-server with WebRTC/RTMP/Whip/Whep support
  help            Print this message or the help of the given subcommand(s)

Options:
      --http-port <HTTP_PORT>  Http port [env: HTTP_PORT=] [default: 3000]
      --http-tls               Run http with tls or not [env: HTTP_TLS=]
      --sdn-port <SDN_PORT>    Sdn port [env: SDN_PORT=] [default: 0]
      --sdn-group <SDN_GROUP>  Sdn group [env: SDN_GROUP=] [default: local]
      --node-id <NODE_ID>      Current Node ID [env: NODE_ID=] [default: 1]
      --secret <SECRET>        Cluster Secret Key [env: SECRET=] [default: insecure]
      --seeds <SEEDS>          Neighbors [env: SEEDS=]
  -h, --help                   Print help
  -V, --version                Print version
```

We can config which node-id, secret for all nodes in cluster. We can also config which seeds for each node to connect to other nodes. Seeds list is list of address of node which will connect first for gathering more information about cluster. After connect to seeds, node will automatically connect to other nodes in cluster.

For config zone info, we will config zone group value, which is used for grouping nodes in cluster. For example, we can have some text like asia-01, asia-02, us-01, us02. Or can more specific like asia-singapore, asia-tokyo, us-newyork, us-sanfrancisco. The zone group is used for automatic building network topology without any manual configuration.

![Multi zones](../imgs/multi-zones.excalidraw.png)

We also can config http port and tls for http server, it will be used for some control api.

## Gateway Node

Gateway node is node which will handle first client request connection, it will route request to best media-server node based on protocol type and client ip address. We use maxmindlite database for getting client location.

```bash
Usage: atm0s-media-server gateway [OPTIONS]

Options:
      --lat <LAT>            lat location [env: LAT=] [default: 0]
      --lng <LNG>            lng location [env: LNG=] [default: 0]
      --geoip-db <GEOIP_DB>  maxmind geo-ip db file [env: GEOIP_DB=] [default: ./maxminddb-data/GeoLite2-City.mmdb]
  -h, --help                 Print help
  -V, --version              Print version
```

If we have multi zone cluster, we need config lat, lng for each zone, and geoip-db database path. This will help gateway node route request to best zone based on client location.

## Media Webtc Node

WebRTC node will support WebRTC SDK and Whip, Whep protocol. We have some config for WebRTC node:

```bash
Usage: atm0s-media-server webrtc [OPTIONS]

Options:
      --max-conn <MAX_CONN>  Max conn [env: MAX_CONN=] [default: 100]
  -h, --help                 Print help
  -V, --version              Print version
```

## Media Sip Node

SIP node enable SIP protocol for media-server. We have some config for SIP node:

```bash
Usage: atm0s-media-server sip [OPTIONS] --addr <ADDR>

Options:
      --addr <ADDR>          Sip listen addr, must is a specific addr, not 0.0.0.0 [env: ADDR=]
      --max-conn <MAX_CONN>  Max conn [env: MAX_CONN=] [default: 100]
      --hook-url <HOOK_URL>  Hook url [env: HOOK_URL=] [default: http://localhost:3000/hooks]
  -h, --help                 Print help
  -V, --version              Print version
```

Note that addr must be a specific addr like `192.168.1.66:5060`, not some think like `0.0.0.0:5060`, because we need to know which addr to send to client in SIP header and SDP.

Hook url is url for sending event to external service, we will send event to this url when have some sip event like: auth, register, unregister, invite.

The 

## Media Rtmp Node

RTMP node enable ingress in RTMP protocol. We have some config for RTMP node:

```bash
Usage: atm0s-media-server rtmp [OPTIONS]

Options:
      --port <PORT>          Rtmp port [env: PORT=] [default: 1935]
      --max-conn <MAX_CONN>  Max conn [env: MAX_CONN=] [default: 10]
  -h, --help                 Print help
  -V, --version              Print version
```

## Connector Node

Connector node enable room and peer event handling at external service over message queue. We have some config for Connector node:

```bash
Usage: atm0s-media-server connector [OPTIONS]

Options:
      --mq-uri <MQ_URI>            Message Queue URI in the form of `amqp://user:pass@host:port/vhost` [env: MQ_URI=] [default: nats://localhost:4222]
      --mq-channel <MQ_CHANNEL>    MQ Channel [env: MQ_CHANNEL=] [default: atm0s/event_log]
      --backup-path <BACKUP_PATH>  Filebase backup path for logs [env: BACKUP_PATH=] [default: .atm0s/data/connector-queue]
  -h, --help                       Print help
  -V, --version                    Print version
```

Currently we only support NATS message queue, but we can easily support other message queue like RabbitMQ, Kafka, ... by implement some interface. For persistent data, we use local file for storing data, it can be config by backup-path.