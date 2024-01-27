# RTMP

You can use RTMP to publish to our media server. Currently we dont support transcode, so you need to publish with same resolution and codec with your room.

Prefer codecs:

- Video: H264, baseline profile, bitrate 2500kbps
- Audio: AAC, bitrate 128kbps

URL: `rtmp://{gateway}/live/{token}`