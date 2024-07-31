use derivative::Derivative;
use serde::{Deserialize, Serialize};

use crate::endpoint::PeerId;

#[derive(Derivative, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[derivative(Debug)]
pub struct MessageChannelPacket {
    pub from: PeerId,
    #[derivative(Debug = "ignore")]
    pub data: Vec<u8>,
}
impl MessageChannelPacket {
    pub fn serialize(&self) -> Vec<u8> {
        bincode::serialize(self).expect("should ok")
    }

    pub fn deserialize(data: &[u8]) -> Option<MessageChannelPacket> {
        bincode::deserialize::<Self>(data).ok()
    }
}
