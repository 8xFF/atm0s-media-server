# RTMP

RTMP transport is implemented by using the rml_rtmp crate.

We start a tcp-server inside media-server create, then create a transport from incoming tcp stream.

Each rtmp session is processed in SAN/IO style:

- RtmpSession: will process incoming data, output event or outgoing data. This part is sync, and not related to I/O
- RtmpTransport: will bridge between RtmpSession and TcpStream. This part is async, and related to I/O
