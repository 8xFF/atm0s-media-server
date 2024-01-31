# Logging

This middleware will hooks into endpoint's state change event and sending it to connector service.
All log data type is encode, decode by protocol buffer (packages/protocol/src/media_endpoint_log.proto).

Each log data will have some kind of data:

- client info (ip, user-agent, ...)
- event type (Routing, Connecting, Connected, Disconnected, Error ...)
- metadata