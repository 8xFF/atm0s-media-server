# Audio Mixer

![Audio Mixer](../../imgs/features/audio-mixer.excalidraw.png)

For more info about how we implement the audio mixer, please refer to [Audio Mixer](/user-guide/features/audio-mixer.md) in the User Guide docs.

We split the core virtual mix-minus logic into a separate module, which is the `audio-mixer` module. This module is standalone and can be used in any other application.

For integrating with an endpoint, we implement a middleware for hooking into the endpoint's pipeline.
