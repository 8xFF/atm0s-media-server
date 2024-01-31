# Audio Mixer

For maximum performance and flexibility, the audio mixer is implemented as virtual mix-minus way. Instead of decode then mix with raw audio data, we chose another approach for avoding decoding and encoding, which call virtual mix-minus.

In this way, we will prepares number of output tracks (typical chose 3), then select most highest volume track and binding to output tracks. This way also can be very flexible, for example in spatial space application, when user can select closest speaker to interested list and media-server will select who is selected to output.

We have 2 level of audio mixer:

- Mix all tracks in a room, which is used for normal audio conference or video conference
- Manual mix tracks, which is used for spatial audio (by add_source, remove_source api in sdk)