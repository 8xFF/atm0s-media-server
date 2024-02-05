# RTMP

You can use RTMP to publish to our media server. Currently, we don't support transcoding, so you need to publish with the same resolution and codec that correspond to your viewers.

Preferred codecs:

- Video: H264, baseline profile, bitrate 2500kbps
- Audio: AAC, bitrate 128kbps

URL: `rtmp://{gateway}/live/{token}`

Demo configuration for OBS:

![Config OBS](../../imgs/demo-rtmp-config.png)