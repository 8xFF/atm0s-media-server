# Concepts

## Introduction

atm0s-media-server is a scalable, flexible, and reliable media server designed to meet the needs of modern streaming applications. Whether deployed in a single zone or across multiple zones, atm0s-media-server provides a powerful solution for handling media streams.

In this document, we will explore the key concepts behind atm0s-media-server, its approach to media streaming, how it works, why it is fast, and how to use it effectively.

## Approach

## Key Features

- Scalability: atm0s-media-server can scale both in a single zone and across multiple zones, allowing for handling large volumes of media streams.
- Protocol Support: It supports multiple protocols such as WebRTC, SIP, and RTMP, providing flexibility for different streaming applications.
- Codec Support: atm0s-media-server is compatible with various codecs including VP8, VP9, H264, OPUS, and more, ensuring compatibility with different media formats.
- Versatility: It is designed to fit any stream application, whether it's video conferencing, live streaming, or spatial room applications.
- Ultra Low Latency: The focus of atm0s-media-server is on achieving ultra-low latency, ensuring real-time communication and smooth streaming experiences.
- Distributed Routing: Users in the same room are not required to be routed to the same media server node, allowing for efficient load balancing and improved performance.

## How it works

To ensure smooth operation and seamless integration with the described approaches, atm0s-media-server is designed with a fastest path routing algorithm, leveraging the power of [atm0s-sdn](https://github.com/8xff/atm0s-sdn) as follows:

- Each peer stream is treated as a pub-sub channel within atm0s-sdn.
- Room and peer metadata are stored in a dedicated key-value store within atm0s-sdn.

Let's dive deeper into how it works.

In a streaming application, multiple rooms exist, each containing several peers. Each peer publishes a stream and subscribes to streams from other peers. Additionally, each room and peer can have associated metadata and settings. To efficiently manage this data, we utilize a key-value store. The streams themselves are divided into senders (publishers) and receivers (subscribers), which are processed within the pub-sub channel.

This architecture ensures a smoother operation and seamless handling of media streams within atm0s-media-server.

![How it works](../imgs/architecture/how-it-works.excalidraw.png)

With the above approaches, we can effectively scale both the pub-sub publishers and the number of subscribers, resulting in a smoother operation of the media server. This scalability is achieved through the decentralized pub-sub and key-value store provided by atm0s-sdn.

## Why it fast

Next, let's explore how atm0s-media-server achieves smooth and efficient operation.

Based on atm0s-sdn, each node establishes connections with other nodes and maintains a route table that determines the best path to reach any other node. This route table is continuously updated, allowing for quick adaptation to network changes.

Using the key-value store, we can easily identify the node responsible for a specific channel and send subscription messages directly to that node. This ensures a fast and optimized data path. Additionally, if a node is already subscribed to a channel, it can reuse the existing subscription. This approach minimizes the number of nodes that receive data from the publisher, resulting in:

- Reduced load on the publisher by limiting the number of data transmissions
- Fast and efficient data transmission between the publisher and subscribers

![Why it fast](../imgs/architecture/why-it-fast.excalidraw.png)

## How to use

For using the media-server, you will need the following SDKs or clients:

- Whip/Whep
- WebRTC SDK (JavaScript, React, React Native, etc.)
- SIP client
- RTMP client

When using the WebRTC SDK, you have the most flexible way to create a streaming application. You can:

- Publish a stream as a sender
- Subscribe to any stream as a receiver and switch between streams or disable/enable them.

To control the quality of streams, which is crucial for a streaming application, you can configure the following parameters:

- Priority
- Maximum and minimum spatial resolution
- Maximum and minimum temporal resolution

Based on these parameters, the server will calculate the appropriate bitrate to send to the receiver. The bitrate will determine which layer of simulcast/SVC should be sent to the receiver.
