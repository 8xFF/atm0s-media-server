use libsoxr::{Datatype, IOSpec, QualityFlags, QualityRecipe, QualitySpec, Soxr};

use super::audio_frame::AudioFrameMono;

fn create_soxr(from: u32, to: u32) -> Option<Soxr> {
    let io_spec = IOSpec::new(Datatype::Int16I, Datatype::Int16I);
    let quality_spec = QualitySpec::new(&QualityRecipe::VeryHigh, QualityFlags::HI_PREC_CLOCK);
    Soxr::create(from as f64, to as f64, 1, Some(&io_spec), Some(&quality_spec), None).ok()
}

pub struct Resampler {
    soxr_8k_48k: Soxr,
    soxr_48k_8k: Soxr,
}

impl Resampler {
    pub fn new() -> Self {
        Self {
            soxr_8k_48k: create_soxr(8000, 48000).expect("Should create soxr"),
            soxr_48k_8k: create_soxr(48000, 8000).expect("Should create soxr"),
        }
    }

    pub fn from_8k_to_48k(&mut self, input: &AudioFrameMono<160, 8000>, output: &mut AudioFrameMono<960, 48000>) {
        let (used, _) = self.soxr_8k_48k.process(Some(input.data()), output.buf_mut()).expect("Should process");
        output.set_samples(used * 6);
    }

    pub fn from_48k_to_8k(&mut self, input: &AudioFrameMono<960, 48000>, output: &mut AudioFrameMono<160, 8000>) {
        let (used, _) = self.soxr_48k_8k.process(Some(input.data()), output.buf_mut()).expect("Should process");
        output.set_samples(used / 6);
    }
}

//TODO avoid this unsafe
unsafe impl Sync for Resampler {}
unsafe impl Send for Resampler {}
