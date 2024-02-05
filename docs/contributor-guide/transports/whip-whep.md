# Whip-Whep

Whip/Whep is a subset of the WebRTC protocol. Unlike the WebRTC SDK, Whip and Whep do not require manual handling of remote streams as it is automatically handled by the media server.

## Whip

After whip client connect to media-server, server will create pre-defined tracks for the client.

- Audio track name: audio_main
- Video track name: video_main

## Whep

To enable automatic binding between remote streams and receivers, we create a Whep middleware.
