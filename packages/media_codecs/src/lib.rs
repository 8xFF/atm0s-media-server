//!
//! This module implement decode and encode logic for some codecs
//! Currently all of codec will asume output raw audio in 48k audio
//!

#[cfg(feature = "opus")]
pub mod opus;
#[cfg(feature = "pcma")]
pub mod pcma;
#[cfg(feature = "resample")]
pub mod resample;

pub trait AudioDecoder {
    fn decode(&mut self, in_buf: &[u8], out_buf: &mut [i16]) -> Option<usize>;
}

pub trait AudioEncodder {
    fn encode(&mut self, in_buf: &[i16], out_buf: &mut [u8]) -> Option<usize>;
}

pub struct AudioTranscoder<Decoder, Encoder> {
    decoder: Decoder,
    encoder: Encoder,
    tmp_buf: [i16; 960],
}

impl<Decoder, Encoder> AudioTranscoder<Decoder, Encoder>
where
    Decoder: AudioDecoder,
    Encoder: AudioEncodder,
{
    pub fn new(decoder: Decoder, encoder: Encoder) -> Self {
        Self { decoder, encoder, tmp_buf: [0; 960] }
    }

    pub fn transcode(&mut self, input: &[u8], output: &mut [u8]) -> Option<usize> {
        let raw_samples = self.decoder.decode(input, &mut self.tmp_buf)?;
        self.encoder.encode(&self.tmp_buf[0..raw_samples], output)
    }
}
