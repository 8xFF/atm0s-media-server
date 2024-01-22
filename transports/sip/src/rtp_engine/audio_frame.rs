pub struct AudioFrameMono<const S: usize, const R: usize> {
    data: [i16; S],
    samples: usize,
}

impl<const S: usize, const R: usize> Default for AudioFrameMono<S, R> {
    fn default() -> Self {
        Self { data: [0; S], samples: 0 }
    }
}

impl<const S: usize, const R: usize> AudioFrameMono<S, R> {
    #[allow(unused)]
    pub fn new(data: [i16; S]) -> Self {
        Self { data, samples: 0 }
    }

    pub fn set_samples(&mut self, samples: usize) {
        self.samples = samples;
    }

    pub fn data(&self) -> &[i16] {
        &self.data[0..self.samples]
    }

    pub fn buf_mut(&mut self) -> &mut [i16; S] {
        &mut self.data
    }

    #[allow(unused)]
    pub fn sample_rate(&self) -> usize {
        R
    }
}
