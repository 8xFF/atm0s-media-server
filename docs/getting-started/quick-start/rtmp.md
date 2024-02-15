# RTMP

You can use RTMP to publish to our media server. Currently, we don't support transcoding, so you need to publish with the same resolution and codec that correspond to your viewers.

Preferred codecs:

- Video: H264, baseline profile, bitrate 2500kbps
- Audio: AAC, bitrate 128kbps

URL: `rtmp://{gateway}/live/{token}`

Demo configuration for OBS:

![Config OBS](../../imgs/demo-rtmp-config.png)

Pregenerated token for default secret and room `demo`, peer `publisher`:

```jwt
eyJhbGciOiJIUzI1NiJ9.eyJyb29tIjoiZGVtbyIsInBlZXIiOiJydG1wIiwicHJvdG9jb2wiOiJSdG1wIiwicHVibGlzaCI6dHJ1ZSwic3Vic2NyaWJlIjpmYWxzZSwidHMiOjE3MDM3NTIzMzU2OTV9.Gj0uCxPwqsFfMFLX8Cufrsyhtb7vedNp3GeUtKQCk3s
```
