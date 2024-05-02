/// This file contains the implementation of the `seq_extend` module.
/// It provides functions for extending sequences for avoiding reset when u16 reach MAX.
///
/// Example usage:
/// ```
/// use media_server_utils::RtpSeqExtend;
///
/// let mut extender = RtpSeqExtend::default();
///
/// assert_eq!(extender.generate(1), Some(1));
/// assert_eq!(extender.generate(65535), None);
/// assert_eq!(extender.generate(2), Some(2));
/// assert_eq!(extender.generate(20000), Some(20000));
/// assert_eq!(extender.generate(40000), Some(40000));
/// assert_eq!(extender.generate(65535), Some(65535));
/// assert_eq!(extender.generate(0), Some(65536));
/// ```
///
const RTP_SEQ_CYCLE: u64 = 1 << 16;
const RTP_SEQ_HAFT_CYCLE: u16 = 1 << 15;

#[derive(Default)]
pub struct RtpSeqExtend {
    last_seq: Option<u16>,
    seq_delta: u64,
}

impl RtpSeqExtend {
    /// Generate extended sequence number from u16 to u64.
    ///
    /// This function takes a sequence number as input, represented as a u16, and extends it to a u64.
    /// It is useful when you need to work with larger sequence numbers that cannot be represented by u16.
    ///
    /// # Arguments
    ///
    /// * `seq_number` - The sequence number to be extended, represented as a u16.
    ///
    /// # Returns
    ///
    /// The extended sequence number as a Some(u64). If it is from previous cycle but cannot subtract the delta, it will return None to avoid subtract with overflow.
    ///
    pub fn generate(&mut self, value: u16) -> Option<u64> {
        if let Some(last_seq) = self.last_seq {
            if value > last_seq && value - last_seq > RTP_SEQ_HAFT_CYCLE {
                if (value as u64 + self.seq_delta) > RTP_SEQ_CYCLE {
                    return Some((value as u64 + self.seq_delta) - RTP_SEQ_CYCLE);
                } else {
                    return None;
                }
            }

            if value < last_seq && last_seq - value > RTP_SEQ_HAFT_CYCLE {
                self.seq_delta += RTP_SEQ_CYCLE;
                log::info!("[RtpSeqExtend] extended to next cycle {:?} => {}, new delta: {}", self.last_seq, value, self.seq_delta);
            }
            self.last_seq = Some(value);

            Some(value as u64 + self.seq_delta)
        } else {
            self.last_seq = Some(value);
            Some(value as u64)
        }
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn increasing_in_next_cycle() {
        let mut extender = super::RtpSeqExtend::default();
        assert_eq!(extender.generate(1), Some(1));
        assert_eq!(extender.generate(2), Some(2));
        assert_eq!(extender.generate(30000), Some(30000));
        assert_eq!(extender.generate(50000), Some(50000));
        assert_eq!(extender.generate(65535), Some(65535));
        assert_eq!(extender.generate(0), Some(65536));
        assert_eq!(extender.generate(1), Some(65537));
    }

    #[test]
    fn previous_cycle() {
        let mut extender = super::RtpSeqExtend::default();
        assert_eq!(extender.generate(1), Some(1));
        assert_eq!(extender.generate(2), Some(2));
        assert_eq!(extender.generate(30000), Some(30000));
        assert_eq!(extender.generate(50000), Some(50000));
        assert_eq!(extender.generate(65535), Some(65535));
        assert_eq!(extender.generate(0), Some(65536));
        assert_eq!(extender.generate(1), Some(65537));
        assert_eq!(extender.generate(65535), Some(65535));
        assert_eq!(extender.generate(0), Some(65536));
        assert_eq!(extender.generate(1), Some(65537));
    }

    #[test]
    fn invalid_cycle() {
        let mut extender = super::RtpSeqExtend::default();
        assert_eq!(extender.generate(1), Some(1));
        assert_eq!(extender.generate(65535), None);
    }
}
