use poem_openapi::Enum;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Copy, Clone, Enum)]
pub enum MixMinusAudioMode {
    Disabled,
    AllAudioStreams,
    ManualAudioStreams,
}

#[derive(Serialize, Deserialize, Debug, Enum, PartialEq, Eq)]
pub enum PayloadType {
    VP8,
    VP9,
    H264,
    OPUS,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Enum, PartialEq, Eq)]
pub enum RemoteBitrateControlMode {
    SumBitrateWithClientSide,
    SumBitrateOnly,
    PerStream,
    MaxBitrateOnly,
}
