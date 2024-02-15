# WebRTC

WebRTC is using a SAN I/O library called Str0m.

We have an internal part which takes care of the protocol and transport logic without coupling with I/O. To integrate with I/O and other parts, we have a wrapper called WebrtcTransport, which will process I/O and convert Str0m events to internal events and vice versa.

Currently, we support UDP and SSLTCP.

TODO: STUN client, TURN server
