use media_server_protocol::media::{H264Profile, MediaCodec, MediaLayerBitrate, MediaLayersBitrate, MediaMeta, MediaPacket, Vp9Profile};
use str0m::{
    format::{CodecConfig, CodecSpec},
    media::{Pt, Rid},
    rtp::{vla::VideoLayersAllocation, RtpPacket, Ssrc},
};

mod bit_read;
mod h264;
mod vp8;
mod vp9;

#[derive(Default)]
pub struct RemoteMediaConvert {
    map: smallmap::Map<Pt, MediaCodec>,
    ssrcs: smallmap::Map<Ssrc, u8>,
}

impl RemoteMediaConvert {
    pub fn set_config(&mut self, cfg: &CodecConfig) {
        for param in cfg.params() {
            if let Some(codec) = str0m_codec_convert(param.spec()) {
                self.map.insert(param.pt(), codec);
            }
        }
    }

    /// This method convert rtp to internal media packet.
    /// It convert VideoLayersAllocation ext to simple layers for using for both simulcast and svc
    pub fn convert(&mut self, rtp: RtpPacket) -> Option<MediaPacket> {
        let spatial = if let Some(rid) = rtp.header.ext_vals.rid {
            let layer = rid_to_spatial(&rid);
            if !self.ssrcs.contains_key(&rtp.header.ssrc) {
                self.ssrcs.insert(rtp.header.ssrc, layer);
            }
            Some(layer)
        } else {
            self.ssrcs.get(&rtp.header.ssrc).cloned()
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
                let layers = rtp.header.ext_vals.user_values.get::<VideoLayersAllocation>().map(extract_simulcast).flatten();
                let meta = h264::parse_rtp(&rtp.payload, profile, spatial)?;
                (true, layers, meta)
            }
            MediaCodec::Vp8 => {
                let layers = rtp.header.ext_vals.user_values.get::<VideoLayersAllocation>().map(extract_simulcast).flatten();
                let meta = vp8::parse_rtp(&rtp.payload, spatial)?;
                (true, layers, meta)
            }
            MediaCodec::Vp9(profile) => {
                let layers = rtp.header.ext_vals.user_values.get::<VideoLayersAllocation>().map(extract_svc).flatten();
                let meta = vp9::parse_rtp(&rtp.payload, profile)?;
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
    map: smallmap::Map<MediaCodec, Pt>,
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
