# Whip/Whep

After prepare token, you can access to media server by using Whip or Whep.

- Whip Endpoint: `{gateway}/whip/endpoint`
- Whep Endpoint: `{gateway}/whep/endpoint`

SDKs compatible:

| SDK                   | Status | Link                                             |
| --------------------- | ------ | ------------------------------------------------ |
| medooze/whip-whep-js  | TODO   | [Repo](https://github.com/medooze/whip-whep-js)  |
| Eyevinn/whip          | TODO   | [Repo](https://github.com/Eyevinn/whip)          |
| Eyevinn/webrtc-player | TODO   | [Repo](https://github.com/Eyevinn/webrtc-player) |

We embed some simple examples to show how to use Whip to publish and play a stream and Whep to play a stream.

![Demo Screen](../../imgs/demo-screen.jpg)

## Whip Sample

Access from gateway: `{gateway}/samples/webrtc/whip.html`
Access from webrtc media server: `{gateway}/samples/whip.html`

Whip sample required access to microphone and camera permission, therefore it need to run with https if you want to test with some remote servers. We have 2 options for that:

- Running gateway or media-server under a reverse proxy like NGINX for providing https
- Start gateway or media-server with `--http-tls` for switching to self-signed https server.

If you don't sepecifc cluster secret, you can use the following pregenerated token to publish to room `demo`, peer `publisher`:

```jwt
eyJhbGciOiJIUzI1NiJ9.eyJyb29tIjoiZGVtbyIsInBlZXIiOiJwdWJsaXNoZXIiLCJwcm90b2NvbCI6IldoaXAiLCJwdWJsaXNoIjp0cnVlLCJzdWJzY3JpYmUiOmZhbHNlLCJ0cyI6MTcwMzc1MjI5NDEyMn0.EfRZK7eHMZ-TCG23-jst8TAKVfbiQhX21cxB2mSznAM
```

## Whep Sample

Access from gateway: `{gateway}/samples/webrtc/whep.html`
Access from webrtc media server: `{gateway}/samples/whep.html`

If you don't sepecifc cluster secret, you can use the following pregenerated token to publish to room `demo`, peer not specific:

```jwt
eyJhbGciOiJIUzI1NiJ9.eyJyb29tIjoiZGVtbyIsInBlZXIiOm51bGwsInByb3RvY29sIjoiV2hlcCIsInB1Ymxpc2giOmZhbHNlLCJzdWJzY3JpYmUiOnRydWUsInRzIjoxNzAzNzUyMzE1NTgyfQ.6XS0gyZWJ699BUN0rXtlLH-0SvgtMXJeXIDtJomxnig
```
