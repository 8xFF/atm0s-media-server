use str0m::{bwe::BweKind, channel::ChannelId, format::CodecConfig, media::KeyframeRequestKind};
use transport::RequestKeyframeKind;

use super::{internal::Str0mInput, mid_convert::mid_to_track, mid_history::MidHistory, rtp_packet_convert::RtpPacketConverter};

pub enum Str0mEventConvertError {
    ChannelNotFound,
    RtpInvalid,
    TrackNotFound,
}

#[derive(Default)]
pub struct Str0mEventConvert {
    channels: Vec<ChannelId>,
    mid_history: MidHistory,
    rtp_convert: RtpPacketConverter,
    twcc_bitrate: Option<u64>,
    remb_bitrate: Option<u64>,
}

impl Str0mEventConvert {
    fn process_bitrate(&mut self, kind: BweKind) -> u64 {
        match kind {
            BweKind::Remb(_, bitrate) => {
                self.remb_bitrate = Some(bitrate.as_u64());
                bitrate.as_u64().min(self.twcc_bitrate.unwrap_or(u64::MAX))
            }
            BweKind::Twcc(bitrate) => {
                self.twcc_bitrate = Some(bitrate.as_u64());
                bitrate.as_u64().min(self.remb_bitrate.unwrap_or(u64::MAX))
            }
        }
    }

    pub fn channel_id(&self, index: usize) -> Option<ChannelId> {
        self.channels.get(index).copied()
    }

    pub fn channel_index(&self, id: ChannelId) -> Option<usize> {
        self.channels.iter().position(|&i| i == id)
    }

    pub fn str0m_sync_codec_config(&mut self, config: &CodecConfig) {
        self.rtp_convert.str0m_sync_codec_config(config);
    }

    pub fn str0m_to_internal(&mut self, event: str0m::Event) -> Result<Option<Str0mInput>, Str0mEventConvertError> {
        match event {
            str0m::Event::Connected => Ok(Some(Str0mInput::Connected)),
            str0m::Event::IceConnectionStateChange(e) => Ok(Some(Str0mInput::IceConnectionStateChange(e))),
            str0m::Event::MediaAdded(added) => Ok(Some(Str0mInput::MediaAdded(added.direction, added.mid, added.kind, added.simulcast))),
            str0m::Event::MediaChanged(changed) => Ok(Some(Str0mInput::MediaChanged(changed.direction, changed.mid))),
            str0m::Event::ChannelOpen(id, name) => {
                let index = self.channels.len();
                self.channels.push(id);
                Ok(Some(Str0mInput::ChannelOpen(index, name)))
            }
            str0m::Event::ChannelData(data) => {
                let channel = self.channel_index(data.id).ok_or(Str0mEventConvertError::ChannelNotFound)?;
                Ok(Some(Str0mInput::ChannelData(channel, data.binary, data.data)))
            }
            str0m::Event::ChannelClose(id) => {
                let channel = self.channel_index(id).ok_or(Str0mEventConvertError::ChannelNotFound)?;
                Ok(Some(Str0mInput::ChannelClosed(channel)))
            }
            str0m::Event::PeerStats(_) => Ok(None),
            str0m::Event::MediaIngressStats(_) => Ok(None),
            str0m::Event::MediaEgressStats(_) => Ok(None),
            str0m::Event::EgressBitrateEstimate(kind) => Ok(Some(Str0mInput::EgressBitrateEstimate(self.process_bitrate(kind)))),
            str0m::Event::KeyframeRequest(req) => match req.kind {
                KeyframeRequestKind::Pli => Ok(Some(Str0mInput::KeyframeRequest(req.mid, RequestKeyframeKind::Pli))),
                KeyframeRequestKind::Fir => Ok(Some(Str0mInput::KeyframeRequest(req.mid, RequestKeyframeKind::Fir))),
            },
            str0m::Event::StreamPaused(status) => {
                let track_id = mid_to_track(&status.mid);
                self.mid_history.get(Some(track_id), *(&status.ssrc as &u32));
                log::info!("[Str0mEventConvert] map between track {} and ssrc {}", track_id, status.ssrc);
                Ok(None)
            }
            str0m::Event::RtpPacket(rtp) => {
                let track_id = rtp.header.ext_vals.mid.map(|mid| mid_to_track(&mid));
                let ssrc: &u32 = &rtp.header.ssrc;
                if let Some(track_id) = self.mid_history.get(track_id, *ssrc) {
                    if let Some(pkt) = self.rtp_convert.to_pkt(rtp) {
                        log::trace!("[Str0mEventConvert] on media {}, {}, {}", pkt.codec, pkt.seq_no, pkt.time);
                        Ok(Some(Str0mInput::MediaPacket(track_id, pkt)))
                    } else {
                        Err(Str0mEventConvertError::RtpInvalid)
                    }
                } else {
                    log::warn!("on rtp without mid {}", rtp.header.ssrc);
                    Err(Str0mEventConvertError::TrackNotFound)
                }
            }
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod test {
    use str0m::{bwe::BweKind, media::Mid};

    #[test]
    fn bitrate_statefull() {
        let mut converter = super::Str0mEventConvert::default();

        assert_eq!(converter.process_bitrate(BweKind::Twcc(1000.into())), 1000);
        assert_eq!(converter.process_bitrate(BweKind::Twcc(2000.into())), 2000);

        assert_eq!(converter.process_bitrate(BweKind::Remb(Mid::default(), 5000.into())), 2000);
        assert_eq!(converter.process_bitrate(BweKind::Remb(Mid::default(), 1500.into())), 1500);
        assert_eq!(converter.process_bitrate(BweKind::Remb(Mid::default(), 5000.into())), 2000);

        assert_eq!(converter.process_bitrate(BweKind::Twcc(8000.into())), 5000);
    }
}
