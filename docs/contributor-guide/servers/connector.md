# Connector

The connector server is a special server used for receiving event logs from all other media servers. We can have multiple connector servers, but we only need one connector server to make the system work.

If we have multiple connector servers, the routing algorithm will send each event log to the best (closest) connector server.

We can have multiple connector servers in each zone, and then the routing algorithm will send the event log to the connector server in the same zone.

Each connector server is connected to a message queue, and external services can get event logs from the message queue. Currently, we only support NATS message queue.
