use std::{collections::HashMap, fmt::Debug, hash::Hash};

const SILENT_LEVEL: i8 = -127;
const SWITCH_AUDIO_THRESHOLD: i16 = 30;
/// if no audio pkt received in AUDIO_SLOT_TIMEOUT_MS, set audio level to SILENT_LEVEL
const AUDIO_SLOT_TIMEOUT_MS: u64 = 1000;

struct SourceState {
    last_changed_at: u64,
    slot: Option<usize>,
}

#[allow(unused)]
#[derive(Debug, Clone)]
struct OutputSlotState<Src> {
    audio_level: i8,
    source: Src,
}

/// Implement lightweight audio mixer with mix-minus feature
/// We will select n highest audio-level tracks
pub struct AudioMixer<Src> {
    len: usize,
    sources: HashMap<Src, SourceState>,
    outputs: Vec<Option<OutputSlotState<Src>>>,
}

impl<Src: Debug + Clone + Eq + Hash> AudioMixer<Src> {
    pub fn new(output: usize) -> Self {
        log::info!("[AudioMixer] create new with {output} outputs");

        Self {
            len: 0,
            sources: HashMap::new(),
            outputs: vec![None; output],
        }
    }

    pub fn on_tick(&mut self, now_ms: u64) -> Option<Vec<usize>> {
        let mut clear = vec![];
        self.sources.retain(|k, v| {
            if v.last_changed_at + AUDIO_SLOT_TIMEOUT_MS <= now_ms {
                log::info!("[AudioMixer] del source {:?} after timeout", k);
                if let Some(slot) = v.slot {
                    self.outputs[slot] = None; //clear
                    self.len -= 1;
                    clear.push(slot);
                }
                false
            } else {
                true
            }
        });
        if clear.is_empty() {
            None
        } else {
            Some(clear)
        }
    }

    pub fn on_pkt(&mut self, now_ms: u64, source: Src, audio_level: Option<i8>) -> Option<(usize, bool)> {
        let audio_level = audio_level.unwrap_or(SILENT_LEVEL);
        if let Some(s) = self.sources.get_mut(&source) {
            s.last_changed_at = now_ms;
            if let Some(slot) = s.slot {
                Some((slot, false))
            } else if self.has_empty_slot() {
                let slot = self.find_empty_slot().expect("Should have empty");
                log::info!("[AudioMixer] switch empty slot {} to source {:?}", slot, source);
                self.sources.get_mut(&source).expect("Should have source").slot = Some(slot);
                self.outputs[slot] = Some(OutputSlotState { audio_level, source });
                self.len += 1;

                Some((slot, true))
            } else {
                //We allway have lowest pin_slot here because above check dont have empty_slot
                let (lowest_index, lowest_source, lowest_audio_level) = self.lowest_slot().expect("Should have lowest pined");
                if lowest_source != source && audio_level as i16 >= lowest_audio_level as i16 + SWITCH_AUDIO_THRESHOLD {
                    log::info!(
                        "[AudioMixer] switch slot {} from source {:?} to source {:?} with higher audio_level",
                        lowest_index,
                        lowest_source,
                        source
                    );
                    self.sources.get_mut(&source).expect("Should have source").slot = Some(lowest_index);
                    self.sources.get_mut(&lowest_source).expect("Should have lowest_source").slot = None;
                    self.outputs[lowest_index] = Some(OutputSlotState { audio_level, source: source.clone() });
                    Some((lowest_index, true))
                } else {
                    None
                }
            }
        } else if let Some(slot) = self.find_empty_slot() {
            log::info!("[AudioMixer] switch empty slot {} to source {:?}", slot, source);
            self.sources.insert(
                source.clone(),
                SourceState {
                    last_changed_at: now_ms,
                    slot: Some(slot),
                },
            );
            self.outputs[slot] = Some(OutputSlotState { audio_level, source });
            self.len += 1;
            Some((slot, true))
        } else {
            log::info!("[AudioMixer] new source {:?}", source);
            self.sources.insert(source.clone(), SourceState { last_changed_at: now_ms, slot: None });
            None
        }
    }

    fn find_empty_slot(&self) -> Option<usize> {
        for (i, slot) in self.outputs.iter().enumerate() {
            if slot.is_none() {
                return Some(i);
            }
        }
        None
    }

    fn has_empty_slot(&self) -> bool {
        self.len < self.outputs.len()
    }

    fn lowest_slot(&self) -> Option<(usize, Src, i8)> {
        let mut lowest: Option<(usize, Src, i8)> = None;
        for (i, slot) in self.outputs.iter().enumerate() {
            if let Some(OutputSlotState { audio_level, source }) = slot {
                if let Some((_, _, lowest_slot_audio_level)) = &mut lowest {
                    if *audio_level < *lowest_slot_audio_level || (*audio_level == *lowest_slot_audio_level) {
                        lowest = Some((i, source.clone(), *audio_level));
                    }
                } else {
                    lowest = Some((i, source.clone(), *audio_level));
                }
            }
        }
        lowest
    }
}

#[cfg(test)]
mod tests {
    use super::{AudioMixer, AUDIO_SLOT_TIMEOUT_MS, SWITCH_AUDIO_THRESHOLD};

    #[test]
    fn add_remove_correct() {
        let mut mixer = AudioMixer::<u32>::new(2);
        assert_eq!(mixer.on_pkt(0, 100, Some(10)), Some((0, true)));
        assert_eq!(mixer.on_pkt(0, 101, Some(10)), Some((1, true)));
        assert_eq!(mixer.on_pkt(0, 102, Some(10)), None);

        assert_eq!(mixer.on_pkt(10, 100, Some(10)), Some((0, false)));
        assert_eq!(mixer.on_pkt(10, 101, Some(10)), Some((1, false)));
        assert_eq!(mixer.on_pkt(10, 102, Some(10)), None);

        assert_eq!(mixer.on_tick(AUDIO_SLOT_TIMEOUT_MS), None);
    }

    #[test]
    fn auto_remove_timeout_source() {
        let mut mixer = AudioMixer::<u32>::new(1);
        assert_eq!(mixer.on_pkt(0, 100, Some(10)), Some((0, true)));
        assert_eq!(mixer.on_pkt(0, 101, Some(10)), None);

        assert_eq!(mixer.on_tick(100), None);
        assert_eq!(mixer.on_pkt(100, 101, Some(10)), None);

        assert_eq!(mixer.on_tick(AUDIO_SLOT_TIMEOUT_MS), Some(vec![0])); //source 100 will be released
        assert_eq!(mixer.on_pkt(100, 101, Some(10)), Some((0, true)));
    }

    #[test]
    fn auto_switch_higher_source() {
        let mut mixer = AudioMixer::<u32>::new(1);
        assert_eq!(mixer.on_pkt(0, 100, Some(10)), Some((0, true)));
        assert_eq!(mixer.on_pkt(0, 101, Some(10)), None);

        assert_eq!(mixer.on_tick(100), None);
        assert_eq!(mixer.on_pkt(100, 100, Some(10)), Some((0, false)));
        assert_eq!(mixer.on_pkt(100, 101, Some(10)), None);

        assert_eq!(mixer.on_tick(200), None); //source 100 will be released
        assert_eq!(mixer.on_pkt(200, 100, Some(10)), Some((0, false)));
        assert_eq!(mixer.on_pkt(200, 101, Some(10 + SWITCH_AUDIO_THRESHOLD as i8)), Some((0, true)));
    }
}
