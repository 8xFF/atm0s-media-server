# Third party system hook

A third-party system hook is a provider that sends internal events from the media server to other systems. The events sent through the hook contain session, peer, and track.

## Usage

The `connector` node sends the hook. So, to enable the hook to provide, you need to use `--hook-uri` to pass the provider's URI when starting the node.

```bash
RUST_LOG=info \
RUST_BACKTRACE=1 \
cargo run -- \
    --sdn-zone-id 0 \
    --sdn-zone-node-id 4 \
    --seeds 1@/ip4/127.0.0.1/udp/10001 \
    connector \
        --hook-uri "http://localhost:30798/webhook"
```

## Message format

Message will sent to another system by using JSON (serde and serde_json) or Binary format which is generated from Protobuf, defined by HookEvent message:

```protobuf
message HookEvent {
    uint32 node = 1;
    uint64 ts = 2;
    oneof event {
        RoomEvent room = 3;
        PeerEvent peer = 4;
        RecordEvent record = 5;
    }
}
```

Example with Json:

```json
{
  "node":1,
  "ts":1724605969302,
  "event":{
    "Peer":{
      "session_id":3005239549225289700,
      "event":{
        "RouteBegin":{
          "remote_ip":"127.0.0.1"
        }
      }
    }
  }
```

## Supported Provider

| provider | status               | description                                             |
| -------- | -------------------- | ------------------------------------------------------- |
| webhook  | :white_check_mark:   | Will send each event using Restful API with POST method |
