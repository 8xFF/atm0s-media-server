# Audio Mixer

![Audio Mixer](../../imgs/features/audio-mixer.excalidraw.png)

For maximum performance and flexibility, the audio mixer is implemented as a virtual mix-minus way. Instead of decoding then mixing with raw audio data, we chose another approach to avoid decoding and encoding, which is called virtual mix-minus.

In this way, we will prepare a number of output tracks (typically choose 3), then select the track with the highest volume and bind it to the output tracks. This way is also very flexible, for example in spatial space application, when the user can select the closest speaker to the interested list and the media-server will select who is selected to output.

We have 2 levels of audio mixer:

- Mix all tracks in a room, which is used for normal audio conference or video conference
- Manual mix tracks, which is used for spatial audio (by using the add_source and remove_source APIs in the SDK)
