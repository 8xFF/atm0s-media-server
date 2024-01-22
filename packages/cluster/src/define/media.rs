use poem_openapi::Enum;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Copy, Clone, Enum)]
pub enum MixMinusAudioMode {
    Disabled,
    AllAudioStreams,
    ManualAudioStreams,
}

impl Default for MixMinusAudioMode {
    fn default() -> Self {
        MixMinusAudioMode::Disabled
    }
}

#[derive(Serialize, Deserialize, Debug, Enum, PartialEq, Eq, Clone)]
pub enum PayloadType {
    VP8,
    VP9,
    H264,
    OPUS,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, Enum, PartialEq, Eq)]
pub enum BitrateControlMode {
    DynamicWithConsumers,
    MaxBitrateOnly,
}

impl Default for BitrateControlMode {
    fn default() -> Self {
        BitrateControlMode::DynamicWithConsumers
    }
}
