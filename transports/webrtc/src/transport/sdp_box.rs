//! This struct implement SdpBox logic for getting data from offer sdp, and rewrite answer sdp
//! The reason of this struct is because SDK need to mapping between stream_id and mid index

enum MediaType {
    Audio,
    Video,
    Data,
}

enum Direction {
    RecvOnly,
    SendOnly,
    SendRecv,
}

pub enum SdpBoxRewriteScope {
    OnlyTrack,
    StreamAndTrack,
}

impl SdpBoxRewriteScope {
    fn generate_uuid(&self, kind: MediaType, index: i32) -> Option<(String, String)> {
        let prefix = match kind {
            MediaType::Audio => Some("audio_"),
            MediaType::Video => Some("video_"),
            _ => None,
        }?;

        match &self {
            SdpBoxRewriteScope::OnlyTrack => Some(("main_stream".to_string(), format!("{}{}", prefix, index))),
            SdpBoxRewriteScope::StreamAndTrack => Some((format!("{}{}", prefix, index), format!("{}{}", prefix, index))),
        }
    }
}

pub struct SdpBox {
    pub scope: SdpBoxRewriteScope,
}

impl SdpBox {
    pub fn rewrite_answer(&mut self, answer: &str) -> String {
        let mut media_type = None;
        let mut direction = None;
        let mut audio_index = -1;
        let mut video_index = -1;

        let mut new_media_lines = answer
            .lines()
            .map(|line| {
                if line.starts_with("m=audio ") {
                    media_type = Some(MediaType::Audio);
                    direction = None;
                } else if line.starts_with("m=video ") {
                    media_type = Some(MediaType::Video);
                    direction = None;
                } else if line.starts_with("m=application ") {
                    media_type = Some(MediaType::Data);
                    direction = None;
                } else if line.starts_with("a=msid:") {
                    if matches!(direction, Some(Direction::SendOnly)) {
                        match media_type {
                            Some(MediaType::Audio) => {
                                let (stream, track) = self.scope.generate_uuid(MediaType::Audio, audio_index).expect("Must success");
                                return format!("a=msid:{stream} {track}");
                            }
                            Some(MediaType::Video) => {
                                let (stream, track) = self.scope.generate_uuid(MediaType::Video, video_index).expect("Must success");
                                return format!("a=msid:{stream} {track}");
                            }
                            _ => {}
                        }
                    }
                } else if line.starts_with("a=ssrc:") {
                    if matches!(direction, Some(Direction::SendOnly)) {
                        let parts = line.split(" ").collect::<Vec<&str>>();
                        if parts.len() == 2 && parts[1].starts_with("cname:") {
                            match media_type {
                                Some(MediaType::Audio) => {
                                    let (stream, _track) = self.scope.generate_uuid(MediaType::Audio, audio_index).expect("Must success");
                                    return format!("{} cname:{stream}", parts[0]);
                                }
                                Some(MediaType::Video) => {
                                    let (stream, _track) = self.scope.generate_uuid(MediaType::Video, video_index).expect("Must success");
                                    return format!("{} cname:{stream}", parts[0]);
                                }
                                _ => {}
                            }
                        } else if parts.len() == 3 && parts[1].starts_with("msid:") {
                            match media_type {
                                Some(MediaType::Audio) => {
                                    let (stream, track) = self.scope.generate_uuid(MediaType::Audio, audio_index).expect("Must success");
                                    return format!("{} msid:{stream} {track}", parts[0]);
                                }
                                Some(MediaType::Video) => {
                                    let (stream, track) = self.scope.generate_uuid(MediaType::Video, video_index).expect("Must success");
                                    return format!("{} msid:{stream} {track}", parts[0]);
                                }
                                _ => {}
                            }
                        }
                    }
                } else {
                    match line {
                        "a=recvonly" => {
                            direction = Some(Direction::RecvOnly);
                        }
                        "a=sendonly" => {
                            direction = Some(Direction::SendOnly);
                            match media_type {
                                Some(MediaType::Audio) => {
                                    audio_index += 1;
                                }
                                Some(MediaType::Video) => {
                                    video_index += 1;
                                }
                                _ => {}
                            }
                        }
                        "a=sendrecv" => {
                            direction = Some(Direction::SendRecv);
                        }
                        _ => {}
                    }
                }
                line.to_string()
            })
            .collect::<Vec<String>>();

        if let Some(last) = new_media_lines.last_mut() {
            last.push_str("\r\n");
        }

        new_media_lines.join("\r\n")
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn stream_track_generator() {
        assert_eq!(
            super::SdpBoxRewriteScope::OnlyTrack.generate_uuid(super::MediaType::Audio, 0),
            Some(("main_stream".to_string(), "audio_0".to_string()))
        );

        assert_eq!(
            super::SdpBoxRewriteScope::OnlyTrack.generate_uuid(super::MediaType::Video, 0),
            Some(("main_stream".to_string(), "video_0".to_string()))
        );

        assert_eq!(
            super::SdpBoxRewriteScope::StreamAndTrack.generate_uuid(super::MediaType::Audio, 0),
            Some(("audio_0".to_string(), "audio_0".to_string()))
        );

        assert_eq!(
            super::SdpBoxRewriteScope::StreamAndTrack.generate_uuid(super::MediaType::Video, 0),
            Some(("video_0".to_string(), "video_0".to_string()))
        );
    }

    #[test]
    fn rewrite_sdp_nothing() {
        let mut sdp_rewrite = super::SdpBox {
            scope: super::SdpBoxRewriteScope::StreamAndTrack,
        };
        assert_eq!(sdp_rewrite.rewrite_answer("v=0\r\n\r\n").as_str(), "v=0\r\n\r\n");
    }

    #[test]
    fn rewrite_sdp_audio_video() {
        let mut sdp_rewrite = super::SdpBox {
            scope: super::SdpBoxRewriteScope::StreamAndTrack,
        };

        let origin = "m=audio 9 UDP/TLS/RTP/SAVPF 111\r\n\
        mid=0\r\n\
        a=sendonly\r\n\
        a=msid:A9A6fAuoozm6l0i72unQhpyhlaHVYQ AofIcgT5LF7fnN0pI2U9senhU5YYHc\r\n\
        a=ssrc:134622262 cname:A9A6fAuoozm6l0i72unQhpyhlaHVYQ\r\n\
        a=ssrc:134622262 msid:A9A6fAuoozm6l0i72unQhpyhlaHVYQ AofIcgT5LF7fnN0pI2U9senhU5YYHc\r\n\
        m=video 9 UDP/TLS/RTP/SAVPF 111\r\n\
        mid=1\r\n\
        a=sendonly\r\n\
        a=msid:A9A6fAuoozm6l0i72unQhpyhlaHVYQ AofIcgT5LF7fnN0pI2U9senhU5YYHc\r\n\
        a=ssrc:134622262 cname:A9A6fAuoozm6l0i72unQhpyhlaHVYQ\r\n\
        a=ssrc:134622262 msid:A9A6fAuoozm6l0i72unQhpyhlaHVYQ AofIcgT5LF7fnN0pI2U9senhU5YYHc\r\n\
        \r\n";
        let expected = "m=audio 9 UDP/TLS/RTP/SAVPF 111\r\n\
        mid=0\r\n\
        a=sendonly\r\n\
        a=msid:audio_0 audio_0\r\n\
        a=ssrc:134622262 cname:audio_0\r\n\
        a=ssrc:134622262 msid:audio_0 audio_0\r\n\
        m=video 9 UDP/TLS/RTP/SAVPF 111\r\n\
        mid=1\r\n\
        a=sendonly\r\n\
        a=msid:video_0 video_0\r\n\
        a=ssrc:134622262 cname:video_0\r\n\
        a=ssrc:134622262 msid:video_0 video_0\r\n\
        \r\n";
        assert_eq!(sdp_rewrite.rewrite_answer(origin).as_str(), expected);
    }
}
