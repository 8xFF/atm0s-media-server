use std::net::{IpAddr, SocketAddr};

use media_server_protocol::transport::{RpcError, RpcResult};
use sdp::{
    description::{
        common::{Address, ConnectionInformation},
        media::{MediaName, RangedPort},
        session::{Origin, TimeDescription, Timing},
    },
    MediaDescription, SessionDescription,
};

use crate::RtpEngineError;

pub fn answer_sdp(sdp: &str, ip: IpAddr, port: u16) -> RpcResult<(String, SocketAddr)> {
    match SessionDescription::try_from(sdp.to_string()) {
        Ok(remote_sdp) => {
            let remote_addr = remote_sdp.connection_information.unwrap().address.unwrap().address;
            let remote_rtp_port = remote_sdp.media_descriptions[0].media_name.port.value;
            let local_sdp = generate_answer(remote_sdp.origin, ip, port as isize);
            let remote_ep: SocketAddr = format!("{}:{}", remote_addr, remote_rtp_port).parse().expect("Unable to parse socket address");
            Ok((local_sdp, remote_ep))
        }
        Err(err) => Err(RpcError::new(RtpEngineError::InvalidSdp, err.to_string().as_str())),
    }
}

fn generate_answer(origin: Origin, local_addr: IpAddr, local_port: isize) -> String {
    let media_description = MediaDescription {
        media_name: MediaName {
            media: "audio".to_string(),
            port: RangedPort { value: local_port, range: None },
            protos: vec!["RTP".to_string(), "AVP".to_string()],
            formats: vec![],
        },
        media_title: None,
        connection_information: None,
        bandwidth: vec![],
        encryption_key: None,
        attributes: vec![],
    }
    .with_codec(106, "opus".to_string(), 48000, 2, "sprop-maxcapturerate=16000; minptime=20; useinbandfec=1".to_string())
    .with_codec(9, "G722".to_string(), 8000, 0, "".to_string())
    .with_codec(0, "PCMU".to_string(), 8000, 0, "".to_string())
    .with_codec(8, "PCMA".to_string(), 8000, 0, "".to_string())
    .with_codec(3, "GSM".to_string(), 8000, 0, "".to_string())
    .with_codec(98, "telephone-event".to_string(), 48000, 0, "0-16".to_string())
    .with_codec(101, "telephone-event".to_string(), 8000, 0, "0-16".to_string())
    .with_property_attribute("sendrecv".to_string());
    let mut sdp = SessionDescription::default().with_media(media_description);
    sdp.session_name = origin.username.clone();
    sdp.origin = origin;
    sdp.connection_information = Some(ConnectionInformation {
        network_type: "IN".to_string(),
        address_type: "IP4".to_string(),
        address: Some(Address {
            address: local_addr.to_string(),
            ttl: None,
            range: None,
        }),
    });
    sdp.time_descriptions = vec![TimeDescription {
        timing: Timing { start_time: 0, stop_time: 0 },
        repeat_times: vec![],
    }];
    sdp.marshal()
}
