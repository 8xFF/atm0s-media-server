# Mix-minus

[Source code](https://github.com/8xFF/atm0s-media-server/blob/master/packages/endpoint/src/endpoint/middleware/mix_minus.rs)

The Mix-minus middleware will create some virtual output tracks and hook events when the endpoint's call switches the receiver to the output tracks.

After that, the middleware will select the track with the highest volume and bind it to the output tracks. The middleware also hooks into the media packet event and outputs it to the output tracks, which will then be sent to the client.

We have 2 modes: mix all tracks in a room and manual mix tracks.

## Mix all tracks in a room mode

In this mode, the middleware will hook into the cluster track added or removed events and automatically subscribe or unsubscribe to the track. All audio data will be sent to the audio mixer, which will select the track with the highest volume and bind it to the output tracks.

## Manual mix tracks mode

In this mode, the client will use the SDK to add or remove tracks to the audio mixer. The audio mixer will subscribe or unsubscribe to the track, and then select the track with the highest volume and bind it to the output tracks.

## Feature improvement

Instead of subscribing to all audio tracks, we can have multiple layers of tracks (metadata, data, etc.). Initially, we only need to subscribe to the first metadata layer (which contains audio level information), and then when we need to mix, we can subscribe to more layers to get the actual audio data.
