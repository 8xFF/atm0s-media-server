# Configuration

The server is designed for simplicity in configuration. We have a single binary that can be configured to run as a gateway, WebRTC node, RTMP node, SIP node, or connector node. The configuration is done through command-line options and very simple without any complex configuration files.

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
      --zone-id <ZONE_ID>  Sdn group [env: ZONE_ID=] [default: 0x000000]
      --node-index <NODE_INDEX>      Current Node Index in zone [env: NODE_INDEX=] [default: 1]
      --secret <SECRET>        Cluster Secret Key [env: SECRET=] [default: insecure]
      --seeds <SEEDS>          Neighbors [env: SEEDS=]
  -h, --help                   Print help
  -V, --version                Print version
```

We can configure the `node-index` and `secret` for all nodes in the cluster. Additionally, we can specify the `seeds` for each node to connect to other nodes. The `seeds` list contains the addresses of nodes that will be connected first to gather more information about the cluster. Once connected to the seeds, the node will automatically connect to other nodes in the cluster.

To configure zone information, we use the `zone-id` opts, which is used for grouping nodes in the cluster. The zone id is used for automatically building the network topology without any manual configuration. (More information about zone configuration can be found in the [Cluster](./features/cluster.md) section.)

![Multi zones](../imgs/multi-zones.excalidraw.png)

Additionally, we have the option to configure the HTTP port and enable TLS for the HTTP server. This is useful for controlling the API.

## Gateway Node

The Gateway node is responsible for handling the initial client connection request. It routes the request to the most suitable media-server node based on the protocol type and client IP address. To determine the client's location, we utilize the MaxMind Lite database.

```bash
Usage: atm0s-media-server gateway [OPTIONS]

Options:
      --lat <LAT>            lat location [env: LAT=] [default: 0]
      --lng <LNG>            lng location [env: LNG=] [default: 0]
      --geoip-db <GEOIP_DB>  maxmind geo-ip db file [env: GEOIP_DB=] [default: ./maxminddb-data/GeoLite2-City.mmdb]
  -h, --help                 Print help
  -V, --version              Print version
```

To enable multi-zone clustering, we can configure the latitude and longitude for each zone, as well as specify the path to the GeoIP database. This allows the gateway node to route client requests to the optimal zone based on their location.

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

Note that the `addr` option must be a specific address like `192.168.1.66:5060`, rather than `0.0.0.0:5060`. This is necessary to determine the address to send to the client in the SIP header and SDP.

The `hook-url` option is the URL for sending events to an external service. Events such as authentication, registration, unregistration, and invitation will be sent to this URL.

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

The Connector node enables room and peer event handling at an external service over a message queue. Here are the configuration options for the Connector node:

```bash
Usage: atm0s-media-server connector [OPTIONS]

Options:
      --mq-uri <MQ_URI>            Message Queue URI in the form of `amqp://user:pass@host:port/vhost` [env: MQ_URI=] [default: nats://localhost:4222]
      --mq-channel <MQ_CHANNEL>    MQ Channel [env: MQ_CHANNEL=] [default: atm0s/event_log]
      --backup-path <BACKUP_PATH>  Filebase backup path for logs [env: BACKUP_PATH=] [default: .atm0s/data/connector-queue]
      --format <FORMAT>            The output format of the message, for now it can either be `protobuf` or `json` [env: FORMAT=] [default: protobuf]
  -h, --help                       Print help
  -V, --version                    Print version
```

Currently, we only support NATS as the message queue. However, we have designed the system to easily support other message queues such as RabbitMQ or Kafka by implementing the necessary interfaces.
You can also use an HTTP API endpoint to receive the cluster events, simply by configuring the MQ URI to be that API Endpoints: `http(s)://localhost:4000/events`. The events will be delivered through a POST request in the specified format. If the format is `protobuf`, the request header will include the content type of `application/octet-stream`.

For persistent data storage, we use local files. You can configure the backup path for storing the data by setting the `backup-path` option.
