use opus_wrap::{Application, Channels};

use crate::{AudioDecoder, AudioEncodder};

#[allow(unused)]
#[allow(clippy::redundant_field_names)]
#[allow(clippy::len_zero)]
#[allow(clippy::needless_lifetimes)]
mod opus_wrap;

pub struct OpusDecoder {
    decoder: opus_wrap::Decoder,
}

impl Default for OpusDecoder {
    fn default() -> Self {
        let decoder = opus_wrap::Decoder::new(48000, Channels::Mono).expect("Should create opus decoder");
        Self { decoder }
    }
}

impl AudioDecoder for OpusDecoder {
    fn decode(&mut self, in_buf: &[u8], out_buf: &mut [i16]) -> Option<usize> {
        //TODO handle fec
        self.decoder.decode(in_buf, out_buf, false).ok()
    }
}

pub struct OpusEncoder {
    encoder: opus_wrap::Encoder,
}

impl Default for OpusEncoder {
    fn default() -> Self {
        let encoder = opus_wrap::Encoder::new(48000, Channels::Mono, Application::Voip).expect("Should create opus encoder");
        Self { encoder }
    }
}

impl AudioEncodder for OpusEncoder {
    fn encode(&mut self, in_buf: &[i16], out_buf: &mut [u8]) -> Option<usize> {
        self.encoder.encode(in_buf, out_buf).ok()
    }
}
