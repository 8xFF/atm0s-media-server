# Logging

[Source code](https://github.com/8xFF/atm0s-media-server/blob/master/packages/endpoint/src/endpoint/middleware/logger.rs)

This middleware will hook into endpoint's state change event and send it to the connector service.
All log data types are encoded and decoded by protocol buffer (packages/protocol/src/media_endpoint_log.proto).

Each log data will have some kind of data:

- client info (ip, user-agent, ...)
- event type (Routing, Connecting, Connected, Disconnected, Error ...)
- metadata
