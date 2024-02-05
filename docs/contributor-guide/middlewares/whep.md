# Whep middleware

Whep is implemented by reusing WebRTC transport, but it need to automatic binding between remote stream and receivers.

[Source code](https://github.com/8xFF/atm0s-media-server/blob/master/servers/media-server/src/server/webrtc/middleware/whep_auto_attach.rs)

We create a weep_auto_attach middleware which hooks into the cluster track added event, then automatically attaches to the track. When the attached track is removed, we will select from the remaining tracks if possible.
