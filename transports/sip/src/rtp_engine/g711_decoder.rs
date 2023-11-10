use super::{
    audio_frame::AudioFrameMono,
    g711::{alaw_to_linear, ulaw_to_linear, G711Codec},
};

pub struct G711Decoder {
    codec: G711Codec,
}

impl G711Decoder {
    pub fn new(codec: G711Codec) -> Self {
        Self { codec }
    }

    pub fn decode(&mut self, input: &[u8], output: &mut AudioFrameMono<160, 8000>) {
        let num_samples = input.len();

        match self.codec {
            G711Codec::Alaw => {
                for (i, sample) in output.buf_mut().iter_mut().enumerate() {
                    *sample = alaw_to_linear(input[i]);
                }
                output.set_samples(num_samples);
            }
            G711Codec::Ulaw => {
                for (i, sample) in output.buf_mut().iter_mut().enumerate() {
                    *sample = ulaw_to_linear(input[i]);
                }
                output.set_samples(num_samples);
            }
        }
    }
}
