# Media Server Core logic

This module implement core logic of media server. It is responsible for:

- Handling data and event from transport layer, and dispatching correct output to cluster
- Handling data and event from cluster, and dispatching correct output to transport layer

## Modules details

- Transport: interact with sdk or clients
- Cluster: interact with atm0s-sdn, convert local action into atm0s-sdn APIs.
- Endpoint: interact with transport in/out and cluster in/out, it act as a bridge between transport and cluster.
- RemoteTrack: track receiving media from client.
- LocalTrack: track sending data to client.

We design each module is very simple responsebility, and interact to each other with general event and control. We need design for avoiding bias into any protocol, instead we design it for room based interactive media server.

Main functions:

- User meta: user state, it can be manual or auto subscribe
- Media stream meta: stream state, it can be manual or auto subscribe
- Media stream: publish, subscribe, unpublish, unsubscribe, bitrate control

### Transport

Transport act and a protocol logic, it can be WebRTC SDK, Whip, Whep or RTMP, SIP or so on. For flexiblity, we should implement transport as a plugin.

Main functions of transport:

- Handle specific protocol data and event: WebRTC, RTMP ..
- Dispatch endpoint generic event (user joined, user leaved, user media ..) to clients with correct protocol.
- Convert protocol events into endpoint generic control event.

### Cluster

Cluster module handle generic event and control from endpoint to ensure it work correct in cluster environtment. Image that endpoint only output very general event like:

- User: user joined, leaved
- RemoteTrack: started, media-data, meta-data, ended
- LocalTrack: started, control, ended

### Endpoint

This is most complex logic, it act for process all of event/data from both cluster and transport. It also take care of process bitrate control, media stream control, user state control.

Controls:

- Room subscribe/unsubscribe
- Other user subscribe/unsubscribe
- User join/leave room
- User publish/unpublish stream
- User subscribe/unsubscribe stream

Cvents

- Room's user joined/leaved room
- User's stream published/unpublished stream
