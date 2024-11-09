use std::collections::HashMap;

use media_server_codecs::{
    opus::{OpusDecoder, OpusEncoder},
    AudioDecoder, AudioEncodder,
};
use media_server_protocol::{
    endpoint::{TrackMeta, TrackName},
    media::MediaPacket,
    transport::RemoteTrackId,
};
use mixer_buffer::MixerBuffer;

mod mixer_buffer;

pub struct AudioMixer {
    tracks: HashMap<(u64, RemoteTrackId), OpusDecoder>,
    audio_tmp: [i16; 960],
    audio_encoded: [u8; 1500],
    mixer: MixerBuffer<(u64, RemoteTrackId)>,
    encoder: OpusEncoder,
}

impl AudioMixer {
    pub fn new() -> Self {
        Self {
            tracks: HashMap::new(),
            audio_tmp: [0; 960],
            audio_encoded: [0; 1500],
            mixer: MixerBuffer::new(),
            encoder: OpusEncoder::default(),
        }
    }

    pub fn add_track(&mut self, session_id: u64, remote_track_id: RemoteTrackId, track_name: TrackName, track_meta: TrackMeta) {
        log::info!("add track {} {} {} {:?}", session_id, remote_track_id, track_name, track_meta);
        self.tracks.insert((session_id, remote_track_id), OpusDecoder::default());
    }

    pub fn on_media(&mut self, session_id: u64, remote_track_id: RemoteTrackId, ts: u64, media_packet: MediaPacket) -> Option<(u64, MediaPacket)> {
        if media_packet.data.is_empty() {
            return None;
        }
        let decoder = self.tracks.get_mut(&(session_id, remote_track_id))?;
        let len = decoder.decode(&media_packet.data, &mut self.audio_tmp)?;
        log::debug!("decode {} {} {} {} => {}", session_id, remote_track_id, media_packet.seq, media_packet.data.len(), len);
        if let Some((ts, frame)) = self.mixer.push(ts, (session_id, remote_track_id), &self.audio_tmp[..len]) {
            let len = self.encoder.encode(&frame, &mut self.audio_encoded)?;
            let media = MediaPacket::build_audio(0, 0, None, self.audio_encoded[..len].to_vec());
            Some((ts, media))
        } else {
            None
        }
    }

    pub fn remove_track(&mut self, session_id: u64, remote_track_id: RemoteTrackId) {
        log::info!("remove track {} {}", session_id, remote_track_id);
        self.tracks.remove(&(session_id, remote_track_id));
    }

    pub fn force_pop(&mut self) -> Option<(u64, MediaPacket)> {
        let (ts, frame) = self.mixer.force_pop()?;
        let len = self.encoder.encode(&frame, &mut self.audio_encoded)?;
        let media = MediaPacket::build_audio(0, 0, None, self.audio_encoded[..len].to_vec());
        Some((ts, media))
    }
}
