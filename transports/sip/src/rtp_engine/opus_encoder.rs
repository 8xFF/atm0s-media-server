use opus::Encoder;

use super::audio_frame::AudioFrameMono;

pub struct OpusEncoder {
    encoder: Encoder,
}

impl OpusEncoder {
    pub fn new() -> Self {
        let mut encoder = Encoder::new(48000, opus::Channels::Mono, opus::Application::Voip).expect("Should create encoder");
        encoder.set_bitrate(opus::Bitrate::Bits(20000));
        encoder.set_inband_fec(true);
        encoder.set_vbr(true);
        Self { encoder }
    }

    pub fn encode(&mut self, input: &AudioFrameMono<960, 48000>, output: &mut [u8]) -> Option<usize> {
        let num_bytes = self.encoder.encode(input.data(), output).expect("Should encode");
        Some(num_bytes)
    }
}
