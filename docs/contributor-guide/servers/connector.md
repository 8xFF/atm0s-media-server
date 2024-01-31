# Connector

Connector server is special server, which is used for receiving event log from all other media-servers. We can have multi connector servers, but we only need one connector server to make system work.

If we have multi connector servers, routing algorithm will send each event log to best (closest) connector server.

We can have multi connector servers at each zone, then routing algorithm will send event log to connector server at same zone.

Each connector server is connected to a message queue, then external service can get event log from the message queue. Currently we only support NATS message queue.