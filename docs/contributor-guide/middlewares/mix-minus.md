# Mix-minus

Mix-minus middleware will create some virtual output tracks, then hook event when endpoint's call switch receiver to the output tracks.

After that, middleware will select most highest volume track and binding to output tracks. The middleware also hook into media packet event and output to the output tracks, then it will be sent into client.

We have 2 modes: mix all tracks in a room, and manual mix tracks.

## Mix all tracks in a room mode

In this mode, middleware will hook into cluster track added or removed event, then automatic subscribe or unsubscribe to the track. All audio data will be sent into audio mixer, then audio mixer will select most highest volume track and binding to output tracks.

## Manual mix tracks mode

In that mode, client will use sdk to add or remove tracks to audio mixer. The audio mixer will subscribe or unsubscribe to the track, then select most highest volume track and binding to output tracks.


## Feature improvement

Instead of subscribe all audio tracks, we can have multi layers track (metadta, data ..), first it only need subscribe first metadata layer (which have audio level info), then when we need to mix, we can subscribe more layer to get actual audio data.