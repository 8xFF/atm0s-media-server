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

#[derive(Default)]
pub struct SdpBox {}

impl SdpBox {
    pub fn rewrite_answer(&mut self, answer: &str) -> String {
        log::info!("before rewrite answer: {}", answer);
        let mut media_type = None;
        let mut direction = None;
        let mut audio_index = 0;
        let mut video_index = 0;

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
                                return format!("a=msid:audio_{} audio_{}", audio_index - 1, audio_index - 1);
                            }
                            Some(MediaType::Video) => {
                                return format!("a=msid:video_{} video_{}", video_index - 1, video_index - 1);
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
                                    return format!("{} cname:audio_{}", parts[0], audio_index - 1);
                                }
                                Some(MediaType::Video) => {
                                    return format!("{} cname:video_{}", parts[0], video_index - 1);
                                }
                                _ => {}
                            }
                        } else if parts.len() == 3 && parts[2].starts_with("msid:") {
                            match media_type {
                                Some(MediaType::Audio) => {
                                    return format!("{} msid:audio_{} audio_{}", parts[0], audio_index - 1, audio_index - 1);
                                }
                                Some(MediaType::Video) => {
                                    return format!("{} msid:video_{} audio_{}", parts[0], video_index - 1, video_index - 1);
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
