use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(tag = "command")]
pub enum NgCommand {
    #[serde(rename = "ping")]
    Ping,

    #[serde(rename = "offer")]
    Offer {
        sdp: String,
        #[serde(rename = "call-id")]
        call_id: String,
        #[serde(rename = "from-tag")]
        from_tag: String,
        #[serde(rename = "ICE")]
        ice: Option<String>,
    },

    #[serde(rename = "answer")]
    Answer {
        sdp: String,
        #[serde(rename = "call-id")]
        call_id: String,
        #[serde(rename = "from-tag")]
        from_tag: String,
        #[serde(rename = "to-tag")]
        to_tag: String,
        #[serde(rename = "ICE")]
        ice: Option<String>,
    },

    #[serde(rename = "delete")]
    Delete {
        #[serde(rename = "call-id")]
        call_id: String,
        #[serde(rename = "from-tag")]
        from_tag: String,
        #[serde(rename = "to-tag")]
        to_tag: Option<String>,
    },
}

impl FromStr for NgCommand {
    type Err = serde_bencode::Error;
    fn from_str(msg: &str) -> Result<Self, Self::Err> {
        serde_bencode::de::from_str(msg)
    }
}

#[allow(clippy::to_string_trait_impl)]
impl ToString for NgCommand {
    fn to_string(&self) -> String {
        serde_bencode::ser::to_string(self).expect("Should convert NgCommand to string")
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(untagged)]
pub enum NgCmdResult {
    Pong {
        result: String,
    },
    Answer {
        result: String,
        sdp: Option<String>,
    },
    Delete {
        result: String,
    },
    Error {
        result: String,
        #[serde(rename = "error-reason")]
        error_reason: String,
    },
}

impl FromStr for NgCmdResult {
    type Err = serde_bencode::Error;
    fn from_str(msg: &str) -> Result<Self, Self::Err> {
        serde_bencode::de::from_str(msg)
    }
}

#[allow(clippy::to_string_trait_impl)]
impl ToString for NgCmdResult {
    fn to_string(&self) -> String {
        serde_bencode::ser::to_string(self).expect("Should convert NgCmdResult to string")
    }
}

#[derive(Debug, Clone)]
pub struct NgRequest {
    pub id: String,
    pub command: NgCommand,
}

impl NgRequest {
    pub fn answer(&self, result: NgCmdResult) -> NgResponse {
        NgResponse { id: self.id.clone(), result }
    }
}

impl FromStr for NgRequest {
    type Err = serde_bencode::Error;
    fn from_str(packet: &str) -> Result<Self, Self::Err> {
        let idx = packet.find(' ');
        match idx {
            Some(idx) => {
                let id = packet[..idx].to_string();
                let body = &packet[idx + 1..];
                Ok(NgRequest {
                    id,
                    command: NgCommand::from_str(body)?,
                })
            }
            None => Err(serde_bencode::Error::MissingField("idx".to_string())),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NgResponse {
    pub id: String,
    pub result: NgCmdResult,
}

impl NgResponse {
    pub fn new(id: String, result: NgCmdResult) -> Self {
        Self { id, result }
    }

    pub fn to_str(&self) -> String {
        let body = serde_bencode::to_string(&self.result).unwrap();
        format!("{} {}", self.id, body)
    }
}

#[cfg(test)]
mod test {

    use std::str::FromStr;

    use super::{NgCmdResult, NgCommand};

    #[test]
    fn ping_command() {
        let actual = NgCommand::Ping {};
        let expect: NgCommand = NgCommand::from_str("d7:command4:pinge").unwrap();

        assert_eq!(expect, actual);
    }

    #[test]
    fn pong_result() {
        assert_eq!(NgCmdResult::Pong { result: "pong".to_string() }, NgCmdResult::from_str("d6:result4:ponge").unwrap());

        assert_eq!(NgCmdResult::Pong { result: "pong".to_string() }.to_string(), "d6:result4:ponge".to_string());
    }

    #[test]
    fn offer_command() {
        let input = "d7:call-id24:bvmWdxbe4hkHHHvCl_d-nQ..7:command5:offer8:from-tag8:460d801e3:sdp3:v=0e";
        let actual = NgCommand::Offer {
            sdp: "v=0".to_string(),
            call_id: "bvmWdxbe4hkHHHvCl_d-nQ..".to_string(),
            from_tag: "460d801e".to_string(),
            ice: None,
        };
        let expect: NgCommand = NgCommand::from_str(input).unwrap();
        assert_eq!(expect, actual);
    }
}
