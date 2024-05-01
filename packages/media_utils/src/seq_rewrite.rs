use sorted_vec::SortedSet;

#[derive(Clone)]
pub struct SeqRewrite<const MAX: u64, const DROPPED_QUEUE_LEN: usize> {
    base: u64,
    max_output: u64,
    max_input: u64,
    seq_delta: u64,
    dropped: SortedSet<u64>,
    reinit: bool,
    reinit_offset: Option<u64>,
}

impl<const MAX: u64, const DROPPED_QUEUE_LEN: usize> Default for SeqRewrite<MAX, DROPPED_QUEUE_LEN> {
    fn default() -> Self {
        Self {
            base: 0,
            max_output: 0,
            max_input: 0,
            seq_delta: 0,
            dropped: SortedSet::with_capacity(DROPPED_QUEUE_LEN),
            reinit: false,
            reinit_offset: None,
        }
    }
}

impl<const MAX: u64, const DROPPED_QUEUE_LEN: usize> SeqRewrite<MAX, DROPPED_QUEUE_LEN> {
    /// Using for marking that we have new stream.
    pub fn reinit(&mut self) {
        self.reinit = true;
    }

    /// Add more offset to base.
    pub fn offset(&mut self, offset: u64) {
        if self.reinit {
            self.reinit_offset = Some(self.reinit_offset.unwrap_or(0) + offset);
        } else {
            self.base = self.wrapping_add(self.base, offset);
        }
    }

    /// Mark the input as dropped.
    pub fn drop_value(&mut self, input: u64) {
        assert!(input < MAX, "{} should < MAX {}", input, MAX);

        if self.reinit {
            self.reinit = false;
            self.sync(self.wrapping_sub(input, 1));
            if let Some(reinit_offset) = self.reinit_offset.take() {
                self.offset(reinit_offset);
            }
        }

        let extended_input = self.extended_seq(input);
        // Mark as dropped if 'input' is higher than anyone already processed.
        if self.is_seq_higher_than(input, self.max_input) {
            self.dropped.push(extended_input);
        }

        // Delete dropped inputs older than input - MaxValue/2.
        if self.dropped.len() > 1000 {
            let delete_size = self.dropped.len() - 1000;
            self.dropped.drain(0..delete_size);
            self.base = self.wrapping_sub(self.base, delete_size as u64);
        }
    }

    /// Using for generate new seq from input seq
    pub fn generate(&mut self, input: u64) -> Option<u64> {
        assert!(input < MAX, "{} should < MAX {}", input, MAX);

        if self.reinit {
            self.reinit = false;
            self.sync(self.wrapping_sub(input, 1));
            if let Some(reinit_offset) = self.reinit_offset.take() {
                self.offset(reinit_offset);
            }
        }

        let extended_input = self.extended_seq(input);
        let mut base = self.base;

        // There are dropped inputs. Synchronize.
        if !self.dropped.is_empty() {
            // Count dropped entries before 'input' in order to adapt the base.
            let mut dropped_count = self.dropped.len();
            match self.dropped.binary_search(&extended_input) {
                Ok(_index) => {
                    return None;
                }
                Err(index) => {
                    dropped_count -= self.dropped.len() - index;
                }
            };

            base = self.wrapping_sub(self.base, dropped_count as u64);
        }

        let output = self.wrapping_add(input, base);

        let idelta = self.wrapping_sub(input, self.max_input);
        let odelta = self.wrapping_sub(output, self.max_output);

        // New input is higher than the maximum seen. But less than acceptable units higher.
        // Keep it as the maximum seen. See Drop().
        if idelta < MAX / 2 {
            self.max_input = input;
        }

        // New output is higher than the maximum seen. But less than acceptable units higher.
        // Keep it as the maximum seen. See Sync().
        if odelta < MAX / 2 {
            self.max_output = output;
        }

        Some(output)
    }

    fn extended_seq(&mut self, value: u64) -> u64 {
        assert!(value < MAX, "{} should < MAX {}", value, MAX);

        if value < self.max_input && self.max_input - value > MAX / 2 {
            self.seq_delta += MAX;
        }
        self.seq_delta + value
    }

    fn is_seq_lower_than(&self, lhs: u64, rhs: u64) -> bool {
        ((rhs > lhs) && (rhs - lhs <= MAX / 2)) || ((lhs > rhs) && (lhs - rhs > MAX / 2))
    }

    fn is_seq_higher_than(&self, lhs: u64, rhs: u64) -> bool {
        ((lhs > rhs) && (lhs - rhs <= MAX / 2)) || ((rhs > lhs) && (rhs - lhs > MAX / 2))
    }

    fn wrapping_sub(&self, v1: u64, v2: u64) -> u64 {
        assert!(v1 < MAX, "{} should < MAX {}", v1, MAX);
        assert!(v2 < MAX, "{} should < MAX {}", v2, MAX);

        if v1 >= v2 {
            v1 - v2
        } else {
            v1 + MAX - v2
        }
    }

    fn wrapping_add(&self, v1: u64, v2: u64) -> u64 {
        (v1 + v2) % MAX
    }

    /// Synchronize the sequence number generator with input.
    /// This function is call when we have new stream with new sequence number.
    fn sync(&mut self, value: u64) {
        assert!(value < MAX, "{} should < MAX {}", value, MAX);

        // Update base.
        self.base = self.wrapping_sub(self.max_output, value);

        // Update maxInput.
        self.max_input = value;

        // Clear seq delta
        self.seq_delta = 0;

        // Clear dropped set.
        self.dropped.clear();
    }
}

#[cfg(test)]
mod test {
    use super::SeqRewrite;

    fn run_test<const MAX: u64>(data: Vec<(u64, u64, bool, bool, Option<u64>)>) {
        let mut rewriter = SeqRewrite::<MAX, 1000>::default();
        for (input, output, reinit, drop, offset) in &data {
            if *reinit {
                rewriter.reinit();
            }

            if let Some(offset) = offset {
                rewriter.offset(*offset);
            }

            if *drop {
                rewriter.drop_value(*input);
            } else {
                assert_eq!(rewriter.generate(*input), Some(*output));
            }
        }
    }

    #[test]
    fn zero_greater_than_65000() {
        const MAX: u64 = u16::MAX as u64 + 1;
        let rw = SeqRewrite::<MAX, 1000>::default();
        assert_eq!(rw.is_seq_higher_than(0, 65000), true);
        assert_eq!(rw.is_seq_lower_than(0, 65000), false);
    }

    #[test]
    fn test_simple() {
        const MAX: u64 = u16::MAX as u64 + 1;
        run_test::<MAX>(vec![
            (0, 0, false, false, None),
            (1, 1, false, false, None),
            (2, 2, false, false, None),
            (3, 3, false, false, None),
            (4, 4, false, false, None),
            (5, 5, false, false, None),
            (6, 6, false, false, None),
            (7, 7, false, false, None),
            (8, 8, false, false, None),
            (9, 9, false, false, None),
            (10, 10, false, false, None),
            (11, 11, false, false, None),
        ]);
    }

    #[test]
    fn ordered_numbers_sync_no_drop() {
        const MAX: u64 = u16::MAX as u64 + 1;
        run_test::<MAX>(vec![
            (0, 0, false, false, None),
            (78, 1, true, false, None), //sync
            (79, 2, false, false, None),
            (80, 3, false, false, None),
            (81, 4, false, false, None),
            (82, 5, false, false, None),
            (83, 6, false, false, None),
            (84, 7, false, false, None),
        ]);
    }

    #[test]
    fn ordered_numbers_sync_with_drop() {
        const MAX: u64 = u16::MAX as u64 + 1;
        run_test::<MAX>(vec![
            (0, 0, false, false, None),
            (78, 0, true, true, None), //sync + drop
            (79, 1, false, false, None),
            (80, 2, false, false, None),
            (81, 3, false, false, None),
            (82, 4, false, false, None),
            (83, 5, false, false, None),
            (84, 6, false, false, None),
        ]);
    }

    #[test]
    fn receive_ordered_numbers_sync_drop() {
        const MAX: u64 = u16::MAX as u64 + 1;
        run_test::<MAX>(vec![
            (0, 0, false, false, None),
            (1, 1, false, false, None),
            (2, 2, false, false, None),
            (3, 3, false, false, None),
            (4, 4, true, false, None), // sync.
            (5, 5, false, false, None),
            (6, 6, false, false, None),
            (7, 7, true, false, None), // sync.
            (8, 0, false, true, None), // drop.
            (9, 8, false, false, None),
            (11, 0, false, true, None), // drop.
            (10, 9, false, false, None),
            (12, 10, false, false, None),
        ]);
    }

    #[test]
    fn receive_ordered_wrapped_numbers() {
        const MAX: u64 = u16::MAX as u64 + 1;
        run_test::<MAX>(vec![
            (65533, 65533, false, false, None),
            (65534, 65534, false, false, None),
            (65535, 65535, false, false, None),
            (0, 0, false, false, None),
            (1, 1, false, false, None),
        ]);
    }

    #[test]
    fn receive_sequence_numbers_with_a_big_jump() {
        const MAX: u64 = u16::MAX as u64 + 1;
        run_test::<MAX>(vec![
            (0, 0, false, false, None),
            (1, 1, false, false, None),
            (1000, 1000, false, false, None),
            (1001, 1001, false, false, None),
        ]);
    }

    #[test]
    fn receive_mixed_numbers_with_a_big_jump_drop_before_jump() {
        const MAX: u64 = u16::MAX as u64 + 1;
        run_test::<MAX>(vec![
            (0, 0, false, false, None),
            (1, 0, false, true, None), // drop.
            (100, 99, false, false, None),
            (100, 99, false, false, None),
            (103, 0, false, true, None), // drop.
            (101, 100, false, false, None),
        ]);
    }

    #[test]
    fn receive_mixed_numbers_with_a_big_jump_drop_after_jump() {
        const MAX: u64 = u16::MAX as u64 + 1;
        run_test::<MAX>(vec![
            (0, 0, false, false, None),
            (1, 1, false, false, None),
            (100, 0, false, true, None), // drop.
            (103, 0, false, true, None), // drop.
            (101, 100, false, false, None),
        ]);
    }

    #[test]
    fn drop_receive_numbers_newer_and_older_than_the_one_dropped() {
        const MAX: u64 = u16::MAX as u64 + 1;
        run_test::<MAX>(vec![
            (0, 0, false, false, None),
            (2, 0, false, true, None), // drop.
            (3, 2, false, false, None),
            (4, 3, false, false, None),
            (1, 1, false, false, None),
        ]);
    }

    #[test]
    fn receive_mixed_numbers_sync_drop() {
        const MAX: u64 = u16::MAX as u64 + 1;
        run_test::<MAX>(vec![
            (0, 0, false, false, None),
            (1, 1, false, false, None),
            (2, 2, false, false, None),
            (3, 3, false, false, None),
            (7, 7, false, false, None),
            (6, 0, false, true, None), // drop.
            (8, 8, false, false, None),
            (10, 10, false, false, None),
            (9, 9, false, false, None),
            (11, 11, false, false, None),
            (0, 12, true, false, None), // sync.
            (2, 14, false, false, None),
            (3, 15, false, false, None),
            (4, 16, false, false, None),
            (5, 17, false, false, None),
            (6, 18, false, false, None),
            (7, 19, false, false, None),
            (8, 20, false, false, None),
            (9, 21, false, false, None),
            (10, 22, false, false, None),
            (9, 0, false, true, None),   // drop.
            (61, 23, true, false, None), // sync.
            (62, 24, false, false, None),
            (63, 25, false, false, None),
            (64, 26, false, false, None),
            (65, 27, false, false, None),
            (11, 28, true, false, None), // sync.
            (12, 29, false, false, None),
            (13, 30, false, false, None),
            (14, 31, false, false, None),
            (15, 32, false, false, None),
            (1, 33, true, false, None), // sync.
            (2, 34, false, false, None),
            (3, 35, false, false, None),
            (4, 36, false, false, None),
            (5, 37, false, false, None),
            (65533, 38, true, false, None), // sync.
            (65534, 39, false, false, None),
            (65535, 40, false, false, None),
            (0, 41, true, false, None), // sync.
            (1, 42, false, false, None),
            (3, 0, false, true, None), // drop.
            (4, 44, false, false, None),
            (5, 45, false, false, None),
            (6, 46, false, false, None),
            (7, 47, false, false, None),
        ]);
    }

    #[test]
    fn receive_ordered_numbers_sync_no_drop_increase_input() {
        const MAX: u64 = u16::MAX as u64 + 1;
        run_test::<MAX>(vec![
            (0, 0, false, false, None),
            (1, 1, false, false, None),
            (2, 2, false, false, None),
            (80, 23, true, false, Some(20)),
            (81, 24, false, false, None),
            (82, 25, false, false, None),
            (83, 26, false, false, None),
            (84, 27, false, false, None),
        ]);
    }

    #[test]
    fn drop_many_inputs_at_the_beginning() {
        const MAX: u64 = u16::MAX as u64 + 1;
        run_test::<MAX>(vec![
            (1, 1, false, false, None),
            (2, 0, false, true, None), // drop.
            (3, 0, false, true, None), // drop.
            (4, 0, false, true, None), // drop.
            (5, 0, false, true, None), // drop.
            (6, 0, false, true, None), // drop.
            (7, 0, false, true, None), // drop.
            (8, 0, false, true, None), // drop.
            (9, 0, false, true, None), // drop.
            (120, 112, false, false, None),
            (121, 113, false, false, None),
            (122, 114, false, false, None),
            (123, 115, false, false, None),
            (124, 116, false, false, None),
            (125, 117, false, false, None),
            (126, 118, false, false, None),
            (127, 119, false, false, None),
            (128, 120, false, false, None),
            (129, 121, false, false, None),
            (130, 122, false, false, None),
            (131, 123, false, false, None),
            (132, 124, false, false, None),
            (133, 125, false, false, None),
            (134, 126, false, false, None),
            (135, 127, false, false, None),
            (136, 128, false, false, None),
            (137, 129, false, false, None),
            (138, 130, false, false, None),
            (139, 131, false, false, None),
        ]);
    }

    #[test]
    fn drop_many_inputs_at_the_beginning_with_high_values() {
        const MAX: u64 = u16::MAX as u64 + 1;
        run_test::<MAX>(vec![
            (1, 1, false, false, None),
            (2, 0, false, true, None), // drop.
            (3, 0, false, true, None), // drop.
            (4, 0, false, true, None), // drop.
            (5, 0, false, true, None), // drop.
            (6, 0, false, true, None), // drop.
            (7, 0, false, true, None), // drop.
            (8, 0, false, true, None), // drop.
            (9, 0, false, true, None), // drop.
            (32768, 32760, false, false, None),
            (32769, 32761, false, false, None),
            (32770, 32762, false, false, None),
            (32771, 32763, false, false, None),
            (32772, 32764, false, false, None),
            (32773, 32765, false, false, None),
            (32774, 32766, false, false, None),
            (32775, 32767, false, false, None),
            (32776, 32768, false, false, None),
            (32777, 32769, false, false, None),
            (32778, 32770, false, false, None),
            (32779, 32771, false, false, None),
            (32780, 32772, false, false, None),
        ]);
    }

    #[test]
    fn sync_and_drop_some_input_near_max_value() {
        const MAX: u64 = u16::MAX as u64 + 1;
        run_test::<MAX>(vec![
            (65530, 1, true, false, None),
            (65531, 2, false, false, None),
            (65532, 3, false, false, None),
            (65533, 0, false, true, None),
            (65534, 0, false, true, None),
            (65535, 4, false, false, None),
            (0, 5, false, false, None),
            (1, 6, false, false, None),
            (2, 7, false, false, None),
            (3, 8, false, false, None),
        ]);
    }

    #[test]
    fn sync_and_drop_some_input_near_max_value_15bit() {
        const MAX: u64 = 0x01_u64 << 15;
        run_test::<MAX>(vec![
            (32762, 1, true, false, None),
            (32763, 2, false, false, None),
            (32764, 3, false, false, None),
            (32765, 0, false, true, None),
            (32766, 0, false, true, None),
            (32767, 4, false, false, None),
            (0, 5, false, false, None),
            (1, 6, false, false, None),
            (2, 7, false, false, None),
            (3, 8, false, false, None),
        ]);
    }
}
