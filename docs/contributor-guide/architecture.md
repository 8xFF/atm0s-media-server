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

Yes, that is only thing we need for implementing streaming application. And we will show how easy it implement with KeyValue and Pubsub mechanism.

TODO: about pubsub and key value usage


## Implementations

For implement above mechanism, the source code is split into

- Transport: for communicate with client, now we have: SIP, RTMP, WebRTC (SDK, Whip, Whep)
- Endpoint: for processing inner logic like rooms, rpc. Inside endpoint we have middleware for implementing more features like: log, audio-mixer, custom behaviour.
- Cluster: for communicate with key-value, pub-sub

TODO: diagram here



