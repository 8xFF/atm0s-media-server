use indexmap::IndexMap;
use media_server_protocol::media::{H264Profile, MediaCodec, MediaLayerBitrate, MediaLayersBitrate, MediaMeta, MediaOrientation, MediaPacket, Vp9Profile};
use str0m::{
    format::{CodecConfig, CodecSpec},
    media::{Mid, Pt, Rid},
    rtp::{vla::VideoLayersAllocation, ExtensionValues, RtpPacket, Ssrc, VideoOrientation},
};

mod bit_read;
mod h264;
mod vp8;
mod vp9;

#[derive(Default)]
pub struct RemoteMediaConvert {
    map: IndexMap<Pt, MediaCodec>,
    ssrcs_rid: IndexMap<Ssrc, u8>,
    ssrcs_mid: IndexMap<Ssrc, Mid>,
}

impl RemoteMediaConvert {
    pub fn set_config(&mut self, cfg: &CodecConfig) {
        for param in cfg.params() {
            if let Some(codec) = str0m_codec_convert(param.spec()) {
                self.map.insert(param.pt(), codec);
            }
        }
    }

    pub fn get_mid(&mut self, ssrc: Ssrc, mid: Option<Mid>) -> Option<Mid> {
        if let Some(mid) = self.ssrcs_mid.get(&ssrc) {
            Some(*mid)
        } else {
            let mid = mid?;
            self.ssrcs_mid.insert(ssrc, mid);
            Some(mid)
        }
    }

    /// This method convert rtp to internal media packet.
    /// It convert VideoLayersAllocation ext to simple layers for using for both simulcast and svc
    pub fn convert(&mut self, rtp: RtpPacket) -> Option<MediaPacket> {
        let spatial = if let Some(rid) = rtp.header.ext_vals.rid {
            let layer = rid_to_spatial(&rid);
            if !self.ssrcs_rid.contains_key(&rtp.header.ssrc) {
                self.ssrcs_rid.insert(rtp.header.ssrc, layer);
            }
            Some(layer)
        } else {
            self.ssrcs_rid.get(&rtp.header.ssrc).cloned()
        };

        let codec = self.remote_pt_to_codec(rtp.header.payload_type)?;
        let (nackable, layers, meta) = match codec {
            MediaCodec::Opus => (
                false,
                None,
                MediaMeta::Opus {
                    audio_level: rtp.header.ext_vals.audio_level,
                },
            ),
            MediaCodec::H264(profile) => {
                let layers = rtp.header.ext_vals.user_values.get::<VideoLayersAllocation>().and_then(extract_simulcast);
                let rotation = rtp.header.ext_vals.video_orientation.map(from_webrtc_orientation);
                let meta = h264::parse_rtp(&rtp.payload, profile, spatial, rotation)?;
                (true, layers, meta)
            }
            MediaCodec::Vp8 => {
                let layers = rtp.header.ext_vals.user_values.get::<VideoLayersAllocation>().and_then(extract_simulcast);
                let rotation = rtp.header.ext_vals.video_orientation.map(from_webrtc_orientation);
                let meta = vp8::parse_rtp(&rtp.payload, spatial, rotation)?;
                (true, layers, meta)
            }
            MediaCodec::Vp9(profile) => {
                let layers = rtp.header.ext_vals.user_values.get::<VideoLayersAllocation>().and_then(extract_svc);
                let rotation = rtp.header.ext_vals.video_orientation.map(from_webrtc_orientation);
                let meta = vp9::parse_rtp(&rtp.payload, profile, rotation)?;
                (true, layers, meta)
            }
        };

        Some(MediaPacket {
            ts: rtp.header.timestamp,
            seq: rtp.header.sequence_number,
            marker: rtp.header.marker,
            nackable,
            layers,
            meta,
            data: rtp.payload,
        })
    }

    fn remote_pt_to_codec(&self, pt: Pt) -> Option<MediaCodec> {
        self.map.get(&pt).cloned()
    }
}

#[derive(Default)]
pub struct LocalMediaConvert {
    map: IndexMap<MediaCodec, Pt>,
}

impl LocalMediaConvert {
    pub fn set_config(&mut self, cfg: &CodecConfig) {
        for param in cfg.params() {
            if let Some(codec) = str0m_codec_convert(param.spec()) {
                self.map.insert(codec, param.pt());
            }
        }
    }

    pub fn convert_codec(&self, codec: MediaCodec) -> Option<Pt> {
        self.map.get(&codec).cloned()
    }

    pub fn rewrite_pkt(&self, pkt: &mut MediaPacket) {
        match &mut pkt.meta {
            MediaMeta::Opus { .. } => {}
            MediaMeta::H264 { sim, .. } => {
                if let Some(sim) = sim {
                    h264::rewrite_rtp(&mut pkt.data, sim);
                }
            }
            MediaMeta::Vp8 { sim, .. } => {
                if let Some(sim) = sim {
                    vp8::rewrite_rtp(&mut pkt.data, sim);
                }
            }
            MediaMeta::Vp9 { svc, .. } => {
                if let Some(svc) = svc {
                    vp9::rewrite_rtp(&mut pkt.data, svc);
                }
            }
        }
    }
}

fn extract_simulcast(vla: &VideoLayersAllocation) -> Option<MediaLayersBitrate> {
    if vla.simulcast_streams.is_empty() {
        return None;
    }

    let mut layers = MediaLayersBitrate::default();
    for (index, sim) in vla.simulcast_streams.iter().enumerate() {
        if let Some(spatial) = sim.spatial_layers.first() {
            let mut layer = MediaLayerBitrate::default();
            for (temporal, bitrate) in spatial.temporal_layers.iter().enumerate() {
                layer.set_layer(temporal, bitrate.cumulative_kbps as u16)
            }
            layers.set_layer(index, layer);
        }
    }
    Some(layers)
}

fn extract_svc(vla: &VideoLayersAllocation) -> Option<MediaLayersBitrate> {
    if vla.simulcast_streams.is_empty() {
        return None;
    }

    let stream = vla.simulcast_streams.first()?;
    let mut layers = MediaLayersBitrate::default();
    let mut previous_bitrate = 0;
    for (spatial, meta) in stream.spatial_layers.iter().enumerate() {
        let mut layer = MediaLayerBitrate::default();
        for (temporal, bitrate) in meta.temporal_layers.iter().enumerate() {
            layer.set_layer(temporal, bitrate.cumulative_kbps as u16 + previous_bitrate)
        }
        previous_bitrate += meta.temporal_layers.last().map(|t| t.cumulative_kbps).unwrap_or(0) as u16;
        layers.set_layer(spatial, layer);
    }

    Some(layers)
}

fn str0m_codec_convert(spec: CodecSpec) -> Option<MediaCodec> {
    match spec.codec {
        str0m::format::Codec::Opus => Some(MediaCodec::Opus),
        str0m::format::Codec::H264 => match (spec.format.profile_level_id, spec.format.packetization_mode) {
            (Some(0x42001f), Some(1)) => Some(MediaCodec::H264(H264Profile::P42001fNonInterleaved)),
            (Some(0x42001f), Some(0)) => Some(MediaCodec::H264(H264Profile::P42001fSingleNal)),
            (Some(0x42e01f), Some(1)) => Some(MediaCodec::H264(H264Profile::P42e01fNonInterleaved)),
            (Some(0x42e01f), Some(0)) => Some(MediaCodec::H264(H264Profile::P42e01fSingleNal)),
            (Some(0x4d001f), Some(1)) => Some(MediaCodec::H264(H264Profile::P4d001fNonInterleaved)),
            (Some(0x4d001f), Some(0)) => Some(MediaCodec::H264(H264Profile::P4d001fSingleNal)),
            (Some(0x64001f), Some(1)) => Some(MediaCodec::H264(H264Profile::P64001fNonInterleaved)),
            _ => {
                log::warn!(
                    "invalid h264 profile_level_id {:?} packetization_mode {:?}",
                    spec.format.profile_level_id,
                    spec.format.packetization_mode
                );
                None
            }
        },
        str0m::format::Codec::Vp8 => Some(MediaCodec::Vp8),
        str0m::format::Codec::Vp9 => match spec.format.profile_id {
            Some(0) => Some(MediaCodec::Vp9(Vp9Profile::P0)),
            Some(2) => Some(MediaCodec::Vp9(Vp9Profile::P2)),
            _ => {
                log::warn!("invalid vp9 profile_id {:?}", spec.format.profile_id);
                None
            }
        },
        _ => None,
    }
}

fn rid_to_spatial(rid: &Rid) -> u8 {
    match rid.as_bytes().first() {
        Some(b'0') => 0,
        Some(b'1') => 1,
        Some(b'2') => 2,
        _ => 0,
    }
}

fn from_webrtc_orientation(orientation: VideoOrientation) -> MediaOrientation {
    match orientation {
        VideoOrientation::Deg0 => MediaOrientation::Deg0,
        VideoOrientation::Deg90 => MediaOrientation::Deg90,
        VideoOrientation::Deg180 => MediaOrientation::Deg180,
        VideoOrientation::Deg270 => MediaOrientation::Deg270,
    }
}

fn to_webrtc_orientation(orientation: MediaOrientation) -> VideoOrientation {
    match orientation {
        MediaOrientation::Deg0 => VideoOrientation::Deg0,
        MediaOrientation::Deg90 => VideoOrientation::Deg90,
        MediaOrientation::Deg180 => VideoOrientation::Deg180,
        MediaOrientation::Deg270 => VideoOrientation::Deg270,
    }
}

pub fn to_webrtc_extensions(pkt: &MediaPacket) -> ExtensionValues {
    let mut ext = ExtensionValues::default();
    if let Some(rotation) = pkt.meta.rotation() {
        ext.video_orientation = Some(to_webrtc_orientation(rotation));
    }
    if let Some(audio_level) = pkt.meta.audio_level() {
        ext.audio_level = Some(audio_level);
    }
    ext
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_str0m_codec_convert() {
        let spec = CodecSpec {
            codec: str0m::format::Codec::Opus,
            clock_rate: str0m::media::Frequency::FORTY_EIGHT_KHZ,
            channels: None,
            format: str0m::format::FormatParams::default(),
        };
        assert_eq!(str0m_codec_convert(spec), Some(MediaCodec::Opus));

        let spec = CodecSpec {
            codec: str0m::format::Codec::H264,
            clock_rate: str0m::media::Frequency::NINETY_KHZ,
            channels: None,
            format: str0m::format::FormatParams::parse_line("profile-level-id=42e01f;packetization-mode=1"),
        };
        assert_eq!(str0m_codec_convert(spec), Some(MediaCodec::H264(H264Profile::P42e01fNonInterleaved)));
    }

    #[test]
    fn test_rid_to_spatial() {
        let rid0 = Rid::from_array([b'0', b'0', b'0', b'0', b'0', b'0', b'0', b'0']);
        assert_eq!(rid_to_spatial(&rid0), 0);

        let rid1 = Rid::from_array([b'1', b'0', b'0', b'0', b'0', b'0', b'0', b'0']);
        assert_eq!(rid_to_spatial(&rid1), 1);

        let rid2 = Rid::from_array([b'2', b'0', b'0', b'0', b'0', b'0', b'0', b'0']);
        assert_eq!(rid_to_spatial(&rid2), 2);

        // other values should be 0
        let rid3 = Rid::from_array([b'3', b'0', b'0', b'0', b'0', b'0', b'0', b'0']);
        assert_eq!(rid_to_spatial(&rid3), 0);
    }

    #[test]
    fn test_from_webrtc_orientation() {
        assert_eq!(from_webrtc_orientation(VideoOrientation::Deg0), MediaOrientation::Deg0);
        assert_eq!(from_webrtc_orientation(VideoOrientation::Deg90), MediaOrientation::Deg90);
        assert_eq!(from_webrtc_orientation(VideoOrientation::Deg180), MediaOrientation::Deg180);
        assert_eq!(from_webrtc_orientation(VideoOrientation::Deg270), MediaOrientation::Deg270);
    }

    #[test]
    fn test_to_webrtc_orientation() {
        assert_eq!(to_webrtc_orientation(MediaOrientation::Deg0), VideoOrientation::Deg0);
        assert_eq!(to_webrtc_orientation(MediaOrientation::Deg90), VideoOrientation::Deg90);
        assert_eq!(to_webrtc_orientation(MediaOrientation::Deg180), VideoOrientation::Deg180);
        assert_eq!(to_webrtc_orientation(MediaOrientation::Deg270), VideoOrientation::Deg270);
    }

    #[test]
    fn test_to_webrtc_extensions() {
        let pkt = MediaPacket::build_audio(1, 1, Some(10), vec![1, 2, 3]);
        let ext = to_webrtc_extensions(&pkt);
        assert_eq!(ext.audio_level, Some(10));

        let pkt = MediaPacket {
            ts: 1,
            seq: 1,
            marker: true,
            nackable: false,
            layers: None,
            meta: MediaMeta::Vp8 {
                key: true,
                sim: None,
                rotation: Some(MediaOrientation::Deg90),
            },
            data: vec![1, 2, 3],
        };
        let ext = to_webrtc_extensions(&pkt);
        assert_eq!(ext.video_orientation, Some(VideoOrientation::Deg90));
    }
}
