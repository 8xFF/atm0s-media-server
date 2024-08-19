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

Message will sent to another system by using JSON format, defined here:

```typescript
{
    uuid: string,
    node: number,
    ts: number,
    event: 'session' | 'peer' | 'remote_track' | 'loacl_track',
    payload: JSON
}
```

Each event will have an individual payload

### Session payload

```typescript
{
    session: number,
    state: 'connecting' | 'connected' | 'reconnect' | 'disconnected' | 'reconnected' | 'connect_error'
    remote_ip: string | null,
    after_ms: number | null
    duration: number | null,
    reason: number | null,
    error: number | null
}
```

### Peer payload

```typescript
{
    session: number,
    peer: string,
    room: string,
    event: 'peer_joined' | 'peer_leaved'
}
```

### Remote track payload

```typescript
{
    session: number
    track: string,
    kind: number,
    event: 'remote_track_started' | 'remote_track_ended'
}
```

### Local track payload

```typescript
{
    session: number,
    track: number,
    event: 'local_track' | 'local_track_attached' | 'local_track_detached',
    kind: number | null,
    remote_peer: string | null,
    remote_track: string | null
}
```

## Supported Provider

| provider | status               | description                                             |
| -------- | -------------------- | ------------------------------------------------------- |
| webhook  | :white_check_mark:   | Will send each event using Restful API with POST method |
| nats     | :white_large_square: |                                                         |
