pub mod data;

use std::net::{IpAddr, SocketAddr};

pub use data::{Codec, CodecSpec, FormatParams, PayloadParams};
use media_server_protocol::transport::{RpcError, RpcResult};
use sdp::{
    description::{
        common::{Address, ConnectionInformation},
        media::{MediaName, RangedPort},
        session::{TimeDescription, Timing},
    },
    MediaDescription, SessionDescription,
};

use crate::RtpEngineError;

#[derive(Debug, Clone, Default)]
pub struct RtpCodecConfig {
    pub params: Vec<PayloadParams>,
}

impl RtpCodecConfig {
    pub fn add_config(&mut self, payload_type: u8, codec: Codec, clock_rate: u32, channels: Option<u16>, format: FormatParams) {
        let p = PayloadParams {
            payload_type,
            spec: CodecSpec { codec, clock_rate, channels, format },
        };
        self.params.push(p);
    }

    pub fn enable_opus(&mut self, enabled: bool) {
        self.params.retain(|c| c.spec.codec != Codec::Opus);
        if !enabled {
            return;
        }

        self.add_config(
            106,
            Codec::Opus,
            48000,
            Some(2),
            FormatParams {
                max_capture_rate: Some(16000),
                min_p_time: Some(20),
                use_inband_fec: Some(true),
                ..Default::default()
            },
        );
    }

    pub fn enable_g722(&mut self, enabled: bool) {
        self.params.retain(|c| c.spec.codec != Codec::G722);
        if !enabled {
            return;
        }

        self.add_config(9, Codec::G722, 8000, None, FormatParams::default());
    }

    pub fn enable_pcmu(&mut self, enabled: bool) {
        self.params.retain(|c| c.spec.codec != Codec::PCMU);
        if !enabled {
            return;
        }

        self.add_config(0, Codec::PCMU, 8000, None, FormatParams::default());
    }

    pub fn enable_pcma(&mut self, enabled: bool) {
        self.params.retain(|c| c.spec.codec != Codec::PCMA);
        if !enabled {
            return;
        }

        self.add_config(8, Codec::PCMA, 8000, None, FormatParams::default());
    }

    pub fn enable_gsm(&mut self, enabled: bool) {
        self.params.retain(|c| c.spec.codec != Codec::GSM);
        if !enabled {
            return;
        }

        self.add_config(3, Codec::GSM, 8000, None, FormatParams::default());
    }

    pub fn enable_telecom_event(&mut self, enabled: bool) {
        self.params.retain(|c| c.spec.codec != Codec::TELEPHONE);
        if !enabled {
            return;
        }

        self.add_config(
            98,
            Codec::TELEPHONE,
            48000,
            None,
            FormatParams {
                dtmf_val: Some(vec![(0, Some(16))]),
                ..Default::default()
            },
        );
        self.add_config(
            101,
            Codec::TELEPHONE,
            8000,
            None,
            FormatParams {
                dtmf_val: Some(vec![(0, Some(16))]),
                ..Default::default()
            },
        );
    }
}

pub struct RtpConfig {
    code_config: RtpCodecConfig,
}

impl RtpConfig {
    pub fn new() -> Self {
        Self {
            code_config: RtpCodecConfig::default(),
        }
    }

    pub fn enable_opus(mut self, enabled: bool) -> Self {
        self.code_config.enable_opus(enabled);
        self
    }

    pub fn enable_g722(mut self, enabled: bool) -> Self {
        self.code_config.enable_g722(enabled);
        self
    }

    pub fn enable_pcmu(mut self, enabled: bool) -> Self {
        self.code_config.enable_pcmu(enabled);
        self
    }

    pub fn enable_pcma(mut self, enabled: bool) -> Self {
        self.code_config.enable_pcma(enabled);
        self
    }

    pub fn enable_gsm(mut self, enabled: bool) -> Self {
        self.code_config.enable_gsm(enabled);
        self
    }

    pub fn enable_telecom_event(mut self, enabled: bool) -> Self {
        self.code_config.enable_telecom_event(enabled);
        self
    }

    pub fn get_config(&self) -> RtpCodecConfig {
        self.code_config.clone()
    }

    pub fn answer(&self, sdp: &str, ip: IpAddr, port: u16) -> RpcResult<(String, SocketAddr)> {
        match SessionDescription::try_from(sdp.to_string()) {
            Ok(remote_sdp) => {
                let media_descs = remote_sdp
                    .media_descriptions
                    .iter()
                    .filter(|m| m.media_name.media == "audio" && m.media_name.protos.iter().any(|p| p == "RTP") && m.media_name.protos.iter().any(|p| p == "AVP"))
                    .map(|m| {
                        let mut answer_media = MediaDescription {
                            media_name: MediaName {
                                media: m.media_name.media.clone(),
                                port: RangedPort { value: port as isize, range: None },
                                protos: m.media_name.protos.clone(),
                                formats: vec![],
                            },
                            media_title: None,
                            connection_information: None,
                            bandwidth: vec![],
                            encryption_key: None,
                            attributes: vec![],
                        };
                        answer_media = self.code_config.params.iter().fold(answer_media, |acc, p| p.add_media_code(acc));
                        answer_media = answer_media.with_property_attribute("sendrecv".to_string());
                        answer_media
                    })
                    .collect::<Vec<_>>();
                let mut sdp = SessionDescription::default();
                for md in media_descs.iter() {
                    sdp = sdp.with_media(md.clone());
                }
                sdp.session_name = remote_sdp.origin.username.clone();
                sdp.origin = remote_sdp.origin;
                sdp.connection_information = Some(ConnectionInformation {
                    network_type: "IN".to_string(),
                    address_type: "IP4".to_string(),
                    address: Some(Address {
                        address: ip.to_string(),
                        ttl: None,
                        range: None,
                    }),
                });
                sdp.time_descriptions = vec![TimeDescription {
                    timing: Timing { start_time: 0, stop_time: 0 },
                    repeat_times: vec![],
                }];
                let answer_sdp = sdp.marshal();
                let remote_addr: SocketAddr = format!("{}:{}", ip, port).parse().expect("Unable to parse socket address");
                Ok((answer_sdp, remote_addr))
            }
            Err(err) => Err(RpcError::new(RtpEngineError::InvalidSdp, err.to_string().as_str())),
        }
    }
}

#[cfg(test)]
mod test {
    use std::net::Ipv4Addr;

    use super::*;

    #[test]
    fn test_generate_answer() {
        let offer = vec![
            "v=0\r\n",
            "o=Zoiper 0 5167452446 IN IP4 113.190.8.4\r\n",
            "s=Zoiper\r\n",
            "c=IN IP4 113.190.8.4\r\n",
            "t=0 0\r\n",
            "m=audio 35415 RTP/AVP 0 101 8 3\r\n",
            "a=rtpmap:101 telephone-event/8000\r\n",
            "a=fmtp:101 0-16\r\n",
            "a=sendrecv\r\n",
            "a=rtcp-mux\r\n",
        ]
        .join("");
        let result = vec![
            "v=0\r\n",
            "o=Zoiper 0 5167452446 IN IP4 113.190.8.4\r\n",
            "s=Zoiper\r\n",
            "c=IN IP4 127.0.0.1\r\n",
            "t=0 0\r\n",
            "m=audio 30001 RTP/AVP 106 9 0 8 3 98 101\r\n",
            "a=rtpmap:106 opus/48000/2\r\n",
            "a=fmtp:106 sprop-maxcapturerate=16000;minptime=20;useinbandfec=1\r\n",
            "a=rtpmap:9 G722/8000\r\n",
            "a=rtpmap:0 PCMU/8000\r\n",
            "a=rtpmap:8 PCMA/8000\r\n",
            "a=rtpmap:3 GSM/8000\r\n",
            "a=rtpmap:98 telephone-event/48000\r\n",
            "a=fmtp:98 0-16\r\n",
            "a=rtpmap:101 telephone-event/8000\r\n",
            "a=fmtp:101 0-16\r\n",
            "a=sendrecv\r\n",
        ]
        .join("");

        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let rtp_config = RtpConfig::new()
            .enable_opus(true)
            .enable_g722(true)
            .enable_pcmu(true)
            .enable_pcma(true)
            .enable_gsm(true)
            .enable_telecom_event(true);
        let (answer, _) = rtp_config.answer(&offer, ip, 30001).expect("error when answer offer");
        assert_eq!(answer, result);
    }
}
