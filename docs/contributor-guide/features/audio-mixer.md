# Audio Mixer

For more info about how we implement audio mixer, please refer to [Audio Mixer](/user-guide/features/audio-mixer.md) in User Guide docs.

We split core virtual mix-minus logic into separate module, which is `audio-mixer` module. This module is a standalone module, which can be used in any other application.

For integrating with endpoint, we implement a middware for hooking into endpoint's pipeline.