use core::fmt;

use sdp::MediaDescription;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Codec {
    Opus,
    G722,
    PCMU,
    PCMA,
    GSM,
    TELEPHONE,
}

impl fmt::Display for Codec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Codec::Opus => write!(f, "opus"),
            Codec::G722 => write!(f, "G722"),
            Codec::PCMU => write!(f, "PCMU"),
            Codec::PCMA => write!(f, "PCMA"),
            Codec::GSM => write!(f, "GSM"),
            Codec::TELEPHONE => write!(f, "telephone-event"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatParam {
    MinPTime(u8),
    UseInbandFec(bool),
    MaxCaptureRate(u32),
    DtmfVal(Vec<(u8, Option<u8>)>),
}

impl fmt::Display for FormatParam {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatParam::MinPTime(v) => write!(f, "minptime={}", v),
            FormatParam::UseInbandFec(v) => write!(f, "useinbandfec={}", *v as u8),
            FormatParam::MaxCaptureRate(v) => write!(f, "sprop-maxcapturerate={}", v),
            FormatParam::DtmfVal(digit) => {
                let s = digit
                    .iter()
                    .map(|(d, v)| {
                        if let Some(v) = v {
                            format!("{}-{}", d, v)
                        } else {
                            format!("{}", d)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                write!(f, "{}", s)
            }
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct FormatParams {
    /// Opus specific parameter
    ///
    /// The minimum duration of media represented by a packet
    pub min_p_time: Option<u8>,

    /// Opus specific parameter
    ///
    /// Specifies that the decoder can do Opus in-band FEC
    pub use_inband_fec: Option<bool>,

    /// Opus specific parameter
    ///
    /// a hint about the maximum input sampling rate
    /// that the sender is likely to produce
    pub max_capture_rate: Option<u32>,

    /// DTMF value
    pub dtmf_val: Option<Vec<(u8, Option<u8>)>>,
}

impl FormatParams {
    pub fn to_format_param(&self) -> Vec<FormatParam> {
        let mut r = Vec::with_capacity(5);
        if let Some(v) = self.max_capture_rate {
            r.push(FormatParam::MaxCaptureRate(v));
        }

        if let Some(v) = self.min_p_time {
            r.push(FormatParam::MinPTime(v));
        }

        if let Some(v) = self.use_inband_fec {
            r.push(FormatParam::UseInbandFec(v));
        }

        if let Some(v) = self.dtmf_val.clone() {
            r.push(FormatParam::DtmfVal(v));
        }

        r
    }
}

impl fmt::Display for FormatParams {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = self.to_format_param().into_iter().map(|f| f.to_string()).collect::<Vec<_>>().join(";");
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone)]
pub struct CodecSpec {
    pub codec: Codec,
    pub clock_rate: u32,
    pub channels: Option<u16>,
    pub format: FormatParams,
}

#[derive(Debug, Clone)]
pub struct PayloadParams {
    pub payload_type: u8,
    pub spec: CodecSpec,
}

impl PayloadParams {
    pub fn add_media_code(&self, desc: MediaDescription) -> MediaDescription {
        desc.with_codec(
            self.payload_type,
            self.spec.codec.to_string(),
            self.spec.clock_rate,
            self.spec.channels.map_or_else(|| 0, |v| v),
            self.spec.format.to_string(),
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn media_codec_str() {
        let codec = Codec::Opus;
        assert_eq!(codec.to_string(), "opus");

        let codec = Codec::G722;
        assert_eq!(codec.to_string(), "G722");

        let codec = Codec::PCMU;
        assert_eq!(codec.to_string(), "PCMU");

        let codec = Codec::PCMA;
        assert_eq!(codec.to_string(), "PCMA");

        let codec = Codec::GSM;
        assert_eq!(codec.to_string(), "GSM");

        let codec = Codec::TELEPHONE;
        assert_eq!(codec.to_string(), "telephone-event");
    }

    #[test]
    fn format_param_str() {
        let param = FormatParam::MinPTime(20);
        assert_eq!(param.to_string(), "minptime=20");

        let param = FormatParam::UseInbandFec(true);
        assert_eq!(param.to_string(), "useinbandfec=true");

        let param = FormatParam::MaxCaptureRate(48000);
        assert_eq!(param.to_string(), "sprop-maxcapturerate=48000");

        let param = FormatParam::DtmfVal(vec![(0, Some(16)), (3, None)]);
        assert_eq!(param.to_string(), "0-16,3");
    }
}
