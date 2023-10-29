/// Helper to replace Bytes. Provides get_u8 and get_u16 over some buffer of bytes.
pub(crate) trait BitRead {
    fn remaining(&self) -> usize;
    fn get_u8(&mut self) -> u8;
    fn get_u16(&mut self) -> u16;
}

impl BitRead for (&[u8], usize) {
    #[inline(always)]
    fn remaining(&self) -> usize {
        (self.0.len() * 8).saturating_sub(self.1)
    }

    #[inline(always)]
    fn get_u8(&mut self) -> u8 {
        if self.remaining() == 0 {
            panic!("Too few bits left");
        }

        let offs = self.1 / 8;
        let shift = (self.1 % 8) as u32;
        self.1 += 8;

        let mut n = self.0[offs];

        if shift > 0 {
            n <<= shift;
            n |= self.0[offs + 1] >> (8 - shift)
        }

        n
    }

    fn get_u16(&mut self) -> u16 {
        u16::from_be_bytes([self.get_u8(), self.get_u8()])
    }
}
