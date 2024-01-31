# Whip-Weep

Whip and weep is implemented by reusing WebRTC transport.

## Whip

We will create a default remote audio track and video track for whip.

- Audio track name: audio_main
- Video track name: video_main

## Weep middleware

We create a weep_auto_attach middleware which hooks into the cluster track added event, then automatically attaches to the track. When the attached track is removed, we will select from the remaining tracks if possible.
