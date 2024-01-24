use serde::{Deserialize, Serialize};

/// This store float f32 as u32, and can be used as key in HashMap
/// ACCURACY is the number of digits after the decimal point
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct F32<const ACCURACY: usize>(u32);

impl<const ACCURACY: usize> F32<ACCURACY> {
    pub fn new(value: f32) -> Self {
        Self((value * 10_f32.powi(ACCURACY as i32)) as u32)
    }

    pub fn value(&self) -> f32 {
        self.0 as f32 / 10_f32.powi(ACCURACY as i32)
    }
}
