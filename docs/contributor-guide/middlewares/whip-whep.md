# Whip-Whep

Whip and whep is implement by reuse Webrtc transport

## Whip

We will create default remote audio track and video track for whip

- Audio track name: audio_main
- Video track name: video_main

## Whep middleware

We create and whep_auto_attach middleware which hook into cluster track added event, then automatic attach to the track. When attached track is removed, we will select from remain tracks if posible.