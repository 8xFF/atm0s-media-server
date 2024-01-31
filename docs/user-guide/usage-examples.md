# Usage Examples

This section provides some usage examples of the atm0s-media-server. If you have a specific use case in mind, please let us know by creating an issue or a pull request.

## Video Conference

A video conference is typically used with at least 2 regions, requiring a multi-zone cluster deployment. The main zone handles the video conference logic, while other zones are used for media server purposes. Additional features such as slides and drawing are handled by the main zone over a WebSocket connection.

![Video conference](../imgs/usecases/video-conference.excalidraw.png)

## CCTV System

Atm0s Media Server can act as a broadcast layer for a CCTV system, allowing it to be viewed on various devices such as web, mobile, and smart TVs. It supports a large number of viewers and can be scaled up by adding more media server nodes in the cloud. The network topology can be simple, working with edge-only nodes, or connected to the cloud for advanced features.

It can also be used to add a CCTV system to a video conference room.

![CCTV system](../imgs/usecases/cctv-extended.excalidraw.png)

## Broadcast

Atm0s Media Server can be used for an ultra-low-latency broadcast system, achieving latency under 500ms. The system can be scaled up using the scale and multi-zone features to accommodate a large number of viewers worldwide.

Ingress can support multiple protocols such as WHIP, RTMP, or WebRTC SDK.
Egress can support multiple protocols such as WHEP or WebRTC SDK.
Media-over-Quic, SRT is considered a future protocol for this use case.

With the help of the smart-routing feature, the publisher is not overloaded by sending data to many nodes, and the data path between the publisher and subscriber is optimized for speed.

![Broadcast](../imgs/usecases/broadcast.excalidraw.png)

## Clubhouse Clone

Similar to the video conference use case, the atm0s-media-server can work in audio-only mode and support mix-minus features. This makes it suitable for hosting large audio rooms without audio transcoding. Only the highest volume audio streams are sent to other peers. Typically, the three highest volume audio streams are chosen to be sent to other peers.
