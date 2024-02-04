# Audio Mixer

![Audio Mixer](../../imgs/features/audio-mixer.excalidraw.png)

For more info about how we implement the audio mixer, please refer to [Audio Mixer](/user-guide/features/audio-mixer.md) in the User Guide docs.

We split the core virtual mix-minus logic into a separate module, which is the `audio-mixer` module. This module is standalone and can be used in any other application.

For integrating with an endpoint, we implement a middleware for hooking into the endpoint's pipeline.

## Abstract Design

We create a new module audio-mixer, which receive all audio stream from other peers and select most loudest audio track to send to client. For flexible, we have 2 modes:

- Mix all audio streams.
- Mix only interesting audio streams.

The number of output tracks can be configured.

## Implementation Details

The core audio-mixer session will have some input and some output:

Input types:

- Source added (source ID)
- Source pkt (source ID, audio level, audio data)
- Source removed (source ID)

Output types:

- Output Pin (output ID, source ID)
- Output pkt (output ID, audio level, audio data)
- Output UnPin (Output ID, source ID)

Each time a source is added the core will check if have free output track, if have, it will pin that output track to that source. When a source is removed, the core will unpin that source from output track.

Each time new audio packet is received, the core will update audio level of that source. If the source wasn't pinned, the core will check if have pined output track wich have lower audio level than that source a threashlod, if have, the core will unpin that output track and pin that source to that output track.

In peridically time, the core will clear audio level of all timeout source, which don't have any audio packet in a period of time.

## Potential Impact and Risks

This method is relized on the fact that the audio level of a source is not changed too much in a short time. If the audio level of a source is changed too much in a short time, the core will unpin that source from output track and pin that output track to another source. This will cause audio glitch.

Another problem is this method is depend on the audio level from source, if the source don't send audio level or send wrong audio level, the core will not work correctly.

## Future improvements

We can split audio stream data to mutli levels, stream metadata (includes audio level) and audio data. We only subscribe with audio data level when we chose a source to mix. This will reduce the bandwidth.

## Open Questions

List any open questions that need to be resolved.

- How to optimize if the room have many sources?
- How to make it smooth when the audio level of a source is changed too much in a short time?
