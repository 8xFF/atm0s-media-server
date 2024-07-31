use libsoxr::{Datatype, IOSpec, QualityFlags, QualityRecipe, QualitySpec, Soxr};

fn create_soxr(from: u32, to: u32) -> Option<Soxr> {
    let io_spec = IOSpec::new(Datatype::Int16I, Datatype::Int16I);
    let quality_spec = QualitySpec::new(&QualityRecipe::VeryHigh, QualityFlags::HI_PREC_CLOCK);
    Soxr::create(from as f64, to as f64, 1, Some(&io_spec), Some(&quality_spec), None).ok()
}

pub struct Resampler<const FROM: u32, const TO: u32> {
    soxr: Soxr,
}

impl<const FROM: u32, const TO: u32> Default for Resampler<FROM, TO> {
    fn default() -> Self {
        Self {
            soxr: create_soxr(FROM, TO).expect("Should create soxr"),
        }
    }
}

impl<const FROM: u32, const TO: u32> Resampler<FROM, TO> {
    pub fn resample(&mut self, input: &[i16], output: &mut [i16]) -> Option<usize> {
        let (_used, generated) = self.soxr.process(Some(input), output).expect("Should process");
        Some(generated)
    }
}
