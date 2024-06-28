use derivative::Derivative;
use serde::{Deserialize, Serialize};

use crate::endpoint::PeerId;

#[derive(Derivative, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[derivative(Debug)]
pub struct DataChannelPacket {
    pub from: PeerId,
    #[derivative(Debug = "ignore")]
    pub data: String,
}
impl DataChannelPacket {
    pub fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).expect("should ok")
    }

    pub fn deserialize(data: &[u8]) -> Option<DataChannelPacket> {
        bincode::deserialize::<Self>(data).ok()
    }
}
