# WebRTC

WebRTC is using SAN I/O libary called str0m.

We have internal part which take care about protocol and transport logic without coupple with I/O. For integrate with I/O and other part, we have wrapper WebrtcTransport which will convert str0m event to internal event and vice versa.

Curretly we support UDP, SSLTCP.

TODO: STUN client, TURN server