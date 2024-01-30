# Architecture

For undestand atm0s-media-server this document will introduce some design approach about it. The document is split to 2 part: abstract design and implements design.

## Abstract design

Different with other media-server is based on single node architecture and manual relay between nodes, atm0s-media-server is designed with global cluster from start. We dont reliaze on any single node or single source of data, you can image that we a have a huge cluster accross many zone which support:

- Key Value store: support HashMap, Set, Del, Sub
- Publish and Subscribe: support publish and subscribe

For any media-server, we only need some base features:

- Client connect to server and join a room
- Client receive room events: peer joined, leaved or stream started, updated, ended
- Client receive stream data from other peers

Next we will show how easy it implement with KeyValue and Pubsub mechanism.

KeyValue store:

- MapID: room identify
- Key: peer identify, stream identify
- Value: peer info, stream info
- Subscriber: each peers in room

PubSub:

- Channel: room + peer + stream identify
- Publisher: the peer who publish stream
- Subscriber: the peers who subscribe stream

Each time a peer joined to room, we will set key-value according to peer info and what stream it published. Each other peers will received event from key-value store and subscribe to stream channel if need. By that way audio and video data will be transfered to the peers.

When a peer leave room, we will remove key-value and unsubscribe from stream channel. Other peers will received event from key-value store and unsubscribe from stream channel if need.

![How it works](/imgs/architecture/how-it-works.excalidraw.png)

About PubSub between nodes, atm0s-sdn overlay network will ensure both thing: bandwidth saving and fast data path.

![Why it fast](/imgs/architecture/why-it-fast.excalidraw.png)

## Implementations

For implement above mechanism, the source code is split into

- Transport: for communicate with client, now we have: SIP, RTMP, WebRTC (SDK, Whip, Whep)
- Endpoint: for processing inner logic like rooms, rpc. Inside endpoint we have middleware for implementing more features like: log, audio-mixer, custom behaviour.
- Cluster: for communicate with key-value, pub-sub

The relationship between them is described in bellow diagram:

![Architecture](/imgs/architecture/implement-layers.excalidraw.png)


### Transport

Transport is create with single trait atm0s-media-server-transport::Transport. The trait is defined as bellow:

```Rust
#[async_trait::async_trait]
pub trait Transport<E, RmIn, RrIn, RlIn, RmOut, RrOut, RlOut> {
    fn on_tick(&mut self, now_ms: u64) -> Result<(), TransportError>;
    fn on_event(&mut self, now_ms: u64, event: TransportOutgoingEvent<RmOut, RrOut, RlOut>) -> Result<(), TransportError>;
    fn on_custom_event(&mut self, now_ms: u64, event: E) -> Result<(), TransportError>;
    async fn recv(&mut self, now_ms: u64) -> Result<TransportIncomingEvent<RmIn, RrIn, RlIn>, TransportError>;
    async fn close(&mut self, now_ms: u64);
}
```

Each transport instance will be managed by endpoint by simple way:

- Endpoint will call on_tick perdicately, example 100ms
- Endpoint will pass event to transport by on_event
- Endpoint will pass custom event to transport by on_custom_event, custom_event is from external source like RPC. Maybe it will be removed in future.
- Endpoint will call recv to get event from transport

The event to Transport is defined as bellow:

```Rust
#[derive(PartialEq, Eq, Debug)]
pub enum TransportOutgoingEvent<RE, RR, RL> {
    RemoteTrackEvent(TrackId, RemoteTrackOutgoingEvent<RR>),
    LocalTrackEvent(TrackId, LocalTrackOutgoingEvent<RL>),
    ConfigEgressBitrate { current: u32, desired: u32 },
    LimitIngressBitrate(u32),
    Rpc(RE),
}
```

The event from Transport is defined as bellow:

```Rust
#[derive(PartialEq, Eq, Debug)]
pub enum TransportIncomingEvent<RE, RR, RL> {
    State(TransportStateEvent),
    Continue,
    RemoteTrackAdded(TrackName, TrackId, TrackMeta),
    RemoteTrackEvent(TrackId, RemoteTrackIncomingEvent<RR>),
    RemoteTrackRemoved(TrackName, TrackId),
    LocalTrackAdded(TrackName, TrackId, TrackMeta),
    LocalTrackEvent(TrackId, LocalTrackIncomingEvent<RL>),
    LocalTrackRemoved(TrackName, TrackId),
    Rpc(RE),
    Stats(TransportStats),
    EgressBitrateEstimate(u64),
}
```

### Endpoint

Endpoint is core logic of atm0s-media-server. It manage how to process event from transport and how to communicate with cluster. The endpoint is designed with SAN IO style, which all logic is independent with I/O and process without async/await. The endpoint is implement inside `packages/endpoint`, can can be defined as bellow:

![Endpoint](/imgs/architecture/endpoint.excalidraw.png)


### Task scheduler

For support large number of peers, we will have a lot of tasks, and for simpler we will only have one task for each endpoint. This is done by `async_std::task::spawn`. The relation between each task is described in bellow diagram:

![Tasks](/imgs/architecture/tasks.excalidraw.png)