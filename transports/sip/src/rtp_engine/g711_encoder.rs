use crate::rtp_engine::g711::{linear_to_alaw, linear_to_ulaw};

use super::{audio_frame::AudioFrameMono, g711::G711Codec};

pub struct G711Encoder {
    codec: G711Codec,
}

impl G711Encoder {
    pub fn new(codec: G711Codec) -> Self {
        Self { codec }
    }

    pub fn encode(&mut self, input: &AudioFrameMono<160, 8000>, output: &mut [u8]) -> Option<usize> {
        let num_bytes = input.data().len();
        assert!(num_bytes <= output.len());
        match self.codec {
            G711Codec::Alaw => {
                for (i, sample) in input.data().iter().enumerate() {
                    output[i] = linear_to_alaw(*sample);
                }
            }
            G711Codec::Ulaw => {
                for (i, sample) in input.data().iter().enumerate() {
                    output[i] = linear_to_ulaw(*sample);
                }
            }
        }
        Some(num_bytes)
    }
}
