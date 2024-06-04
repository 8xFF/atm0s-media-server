//!
//! Audio mixer in room level is splited to 2 part:
//! - Publisher: detect top 3 audio and publish to /room_id/audio_mixer channel
//! - Subscriber: subscribe to /room_id/audio_mixer to get all of top-3 audios from other servers
//!                 calculate top-3 audio for each local endpoint
//!

pub struct AudioMixer {}

impl AudioMixer {}
