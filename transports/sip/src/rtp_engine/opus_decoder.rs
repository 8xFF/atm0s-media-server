use opus::{Channels, Decoder};

use super::audio_frame::AudioFrameMono;

pub struct OpusDecoder {
    decoder: Decoder,
}

impl OpusDecoder {
    pub fn new() -> Self {
        Self {
            decoder: Decoder::new(48000, Channels::Mono).expect("Should create decoder"),
        }
    }

    //TODO working with FEC
    pub fn decode(&mut self, input: &[u8], output: &mut AudioFrameMono<960, 48000>) {
        let num_samples = self.decoder.decode(input, output.buf_mut(), false).expect("Should decode");
        output.set_samples(num_samples);
    }
}
