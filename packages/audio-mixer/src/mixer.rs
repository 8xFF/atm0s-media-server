use std::{
    collections::{HashMap, VecDeque},
    fmt::Debug,
    hash::Hash,
};

const SILENT_LEVEL: i8 = -127;
const SWITCH_AUDIO_THRESHOLD: i8 = 30;
/// if no audio pkt received in AUDIO_SLOT_TIMEOUT_MS, set audio level to SILENT_LEVEL
const AUDIO_SLOT_TIMEOUT_MS: u64 = 1000;

pub struct AudioMixerConfig {
    pub outputs: usize,
}

#[derive(Debug, PartialEq, Eq)]
pub enum AudioMixerOutput<Pkt, Src> {
    SlotPinned(Src, usize),
    SlotUnpinned(Src, usize),
    OutputSlotSrcChanged(usize, Option<Src>),
    OutputSlotPkt(usize, Pkt),
}

enum SourceState {
    Unpinned { audio_level: i8, last_changed_at: u64 },
    Pinned { pinned_at: u64, audio_level: i8, slot: usize, last_changed_at: u64 },
}

impl SourceState {
    fn audio_level(&self) -> i8 {
        match self {
            SourceState::Unpinned { audio_level, .. } => *audio_level,
            SourceState::Pinned { audio_level, .. } => *audio_level,
        }
    }

    fn slot(&self) -> Option<usize> {
        match self {
            SourceState::Unpinned { .. } => None,
            SourceState::Pinned { slot, .. } => Some(*slot),
        }
    }

    fn set_audio_level(&mut self, now_ms: u64, audio_level: i8) {
        match self {
            SourceState::Unpinned {
                audio_level: level, last_changed_at, ..
            } => {
                *level = audio_level;
                *last_changed_at = now_ms;
            }
            SourceState::Pinned {
                audio_level: level, last_changed_at, ..
            } => {
                *level = audio_level;
                *last_changed_at = now_ms;
            }
        }
    }

    fn last_changed_at(&self) -> u64 {
        match self {
            SourceState::Unpinned { last_changed_at, .. } => *last_changed_at,
            SourceState::Pinned { last_changed_at, .. } => *last_changed_at,
        }
    }
}

#[derive(Debug, Clone)]
enum OutputSlotState<Src: Clone> {
    Empty,
    Pinned { pinned_at: u64, audio_level: i8, source: Src, last_changed_at: u64 },
}

impl<Src: Clone> OutputSlotState<Src> {
    pub fn audio_level(&self) -> Option<i8> {
        match self {
            OutputSlotState::Empty => None,
            OutputSlotState::Pinned { audio_level, .. } => Some(*audio_level),
        }
    }
}

/// Implement lightweight audio mixer with mix-minus feature
/// We will select n highest audio-level tracks
pub struct AudioMixer<Pkt: Clone, Src: Debug + Clone + Eq + Hash> {
    extractor: Box<dyn (Fn(&Pkt) -> Option<i8>) + Send + Sync>,
    sources: HashMap<Src, SourceState>,
    output_slots: Vec<OutputSlotState<Src>>,
    actions: VecDeque<AudioMixerOutput<Pkt, Src>>,
}

impl<Pkt: Clone, Src: Debug + Clone + Eq + Hash> AudioMixer<Pkt, Src> {
    pub fn new(extractor: Box<dyn (Fn(&Pkt) -> Option<i8>) + Send + Sync>, config: AudioMixerConfig) -> Self {
        log::info!("[AudioMixer] create new with {} outputs", config.outputs);

        Self {
            extractor,
            sources: HashMap::new(),
            output_slots: vec![OutputSlotState::Empty; config.outputs],
            actions: VecDeque::new(),
        }
    }

    pub fn on_tick(&mut self, now_ms: u64) {
        for (src, state) in &mut self.sources {
            if state.last_changed_at() + AUDIO_SLOT_TIMEOUT_MS <= now_ms {
                log::debug!("[AudioMixer] set source {:?} audio level to SILENT_LEVEL after timeout", src);
                state.set_audio_level(now_ms, SILENT_LEVEL);
                if let Some(slot) = state.slot() {
                    if let OutputSlotState::Pinned { audio_level, last_changed_at, .. } = &mut self.output_slots[slot] {
                        *audio_level = SILENT_LEVEL;
                        *last_changed_at = now_ms;
                    }
                }
            }
        }
    }

    /// add source to mixer
    /// if mixer slot is not full, select a empty slot and pin the source to the slot
    pub fn add_source(&mut self, now_ms: u64, source: Src) {
        if self.sources.contains_key(&source) {
            return;
        }
        if let Some(slot) = self.find_empty_slot() {
            log::info!("[AudioMixer] add source {:?} to slot {}", source, slot);
            self.output_slots[slot] = OutputSlotState::Pinned {
                pinned_at: now_ms,
                audio_level: SILENT_LEVEL,
                source: source.clone(),
                last_changed_at: now_ms,
            };
            self.sources.insert(
                source.clone(),
                SourceState::Pinned {
                    pinned_at: now_ms,
                    audio_level: SILENT_LEVEL,
                    slot,
                    last_changed_at: now_ms,
                },
            );
            self.actions.push_back(AudioMixerOutput::SlotPinned(source.clone(), slot));
            self.actions.push_back(AudioMixerOutput::OutputSlotSrcChanged(slot, Some(source)));
        } else {
            log::info!("[AudioMixer] add source {:?} to mixer but no empty slot", source);
            self.sources.insert(
                source,
                SourceState::Unpinned {
                    audio_level: SILENT_LEVEL,
                    last_changed_at: now_ms,
                },
            );
        }
    }

    /// push pkt to mixer, if audio level is higher than lowest audio level + SWITCH_AUDIO_THRESHOLD, switch the slot to the source
    pub fn push_pkt(&mut self, now_ms: u64, source: Src, pkt: &Pkt) {
        let audio_level = (self.extractor)(pkt).unwrap_or(SILENT_LEVEL);
        if let Some(index) = self.set_source_level(now_ms, source, audio_level) {
            log::trace!("[AudioMixer] push pkt to slot {}, audio level {}", index, audio_level);
            self.actions.push_back(AudioMixerOutput::OutputSlotPkt(index, pkt.clone()));
        }
    }

    /// remove source from mixer
    /// if removed source id pinned, find another source to fill the slot
    pub fn remove_source(&mut self, now_ms: u64, source: Src) {
        if let Some(SourceState::Pinned {
            pinned_at: _,
            audio_level: _,
            slot,
            last_changed_at: _,
        }) = self.sources.remove(&source)
        {
            //find another source to fill the slot
            if let Some((src, state)) = self.highest_unpined_source() {
                log::info!("[AudioMixer] remove source {:?} from slot {}, fill slot with source {:?}", source, slot, src);
                *state = SourceState::Pinned {
                    pinned_at: now_ms,
                    audio_level: state.audio_level(),
                    slot,
                    last_changed_at: now_ms,
                };
                self.output_slots[slot] = OutputSlotState::Pinned {
                    pinned_at: now_ms,
                    audio_level: state.audio_level(),
                    source: src.clone(),
                    last_changed_at: now_ms,
                };
                self.actions.push_back(AudioMixerOutput::SlotUnpinned(source, slot));
                self.actions.push_back(AudioMixerOutput::SlotPinned(src.clone(), slot));
                self.actions.push_back(AudioMixerOutput::OutputSlotSrcChanged(slot, Some(src)));
            } else {
                log::info!("[AudioMixer] remove source {:?} from slot {}, no source to fill slot", source, slot);
                self.output_slots[slot] = OutputSlotState::Empty;
                self.actions.push_back(AudioMixerOutput::OutputSlotSrcChanged(slot, None));
            }
        }
    }

    pub fn pop(&mut self) -> Option<AudioMixerOutput<Pkt, Src>> {
        self.actions.pop_front()
    }

    fn find_empty_slot(&self) -> Option<usize> {
        for (i, slot) in self.output_slots.iter().enumerate() {
            if let OutputSlotState::Empty = slot {
                return Some(i);
            }
        }
        None
    }

    fn highest_unpined_source(&mut self) -> Option<(Src, &mut SourceState)> {
        let mut highest: Option<(Src, &mut SourceState)> = None;
        for (src, state) in self.sources.iter_mut() {
            if let SourceState::Unpinned { audio_level, .. } = state {
                if let Some((_, highest_state)) = &mut highest {
                    if *audio_level > highest_state.audio_level() {
                        highest = Some((src.clone(), state));
                    }
                } else {
                    highest = Some((src.clone(), state));
                }
            }
        }
        highest
    }

    fn lowest_pinned_slot(&self) -> Option<(usize, Src, i8, u64)> {
        let mut lowest: Option<(usize, Src, i8, u64)> = None;
        for (i, slot) in self.output_slots.iter().enumerate() {
            if let OutputSlotState::Pinned {
                audio_level, source, last_changed_at, ..
            } = slot
            {
                if let Some((_, _, lowest_slot_audio_level, lowest_last_changed_at)) = &mut lowest {
                    if *audio_level < *lowest_slot_audio_level || (*audio_level == *lowest_slot_audio_level && *last_changed_at < *lowest_last_changed_at) {
                        lowest = Some((i, source.clone(), *audio_level, *last_changed_at));
                    }
                } else {
                    lowest = Some((i, source.clone(), *audio_level, *last_changed_at));
                }
            }
        }
        lowest
    }

    /// set audio level for source, if source is pinned return Some(slot index), else return None
    /// Each time we compare audio level with lowest pinned audio level,
    /// if audio level is higher than lowest audio level + SWITCH_AUDIO_THRESHOLD,
    /// we switch the slot to the source
    fn set_source_level(&mut self, now_ms: u64, source: Src, level: i8) -> Option<usize> {
        let state = self.sources.get_mut(&source)?;
        match state {
            SourceState::Unpinned { audio_level, last_changed_at, .. } => {
                *audio_level = level;
                *last_changed_at = now_ms;
                if let Some((lowest_index, lowest_source, lowest_audio_level, _lowest_last_changed_at)) = self.lowest_pinned_slot() {
                    if lowest_source != source && level >= lowest_audio_level + SWITCH_AUDIO_THRESHOLD {
                        log::info!(
                            "[AudioMixer] switch slot {} from source {:?} to source {:?} with higher audio_level",
                            lowest_index,
                            lowest_source,
                            source
                        );
                        self.sources.insert(
                            source.clone(),
                            SourceState::Pinned {
                                pinned_at: now_ms,
                                audio_level: level,
                                slot: lowest_index,
                                last_changed_at: now_ms,
                            },
                        );
                        self.output_slots[lowest_index] = OutputSlotState::Pinned {
                            pinned_at: now_ms,
                            audio_level: level,
                            source: source.clone(),
                            last_changed_at: now_ms,
                        };

                        self.actions.push_back(AudioMixerOutput::SlotUnpinned(lowest_source, lowest_index));
                        self.actions.push_back(AudioMixerOutput::SlotPinned(source.clone(), lowest_index));
                        self.actions.push_back(AudioMixerOutput::OutputSlotSrcChanged(lowest_index, Some(source)));

                        Some(lowest_index)
                    } else {
                        log::trace!(
                            "[AudioMixer] set source {:?} audio level to {}, but not higher than lowest audio level {} + SWITCH_AUDIO_THRESHOLD {}",
                            source,
                            level,
                            lowest_audio_level,
                            SWITCH_AUDIO_THRESHOLD
                        );
                        None
                    }
                } else {
                    panic!("should not happen");
                }
            }
            SourceState::Pinned {
                audio_level, slot, last_changed_at, ..
            } => {
                *audio_level = level;
                *last_changed_at = now_ms;
                if let OutputSlotState::Pinned { audio_level, last_changed_at, .. } = &mut self.output_slots[*slot] {
                    *audio_level = level;
                    *last_changed_at = now_ms;
                }
                Some(*slot)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{AudioMixer, AudioMixerConfig};

    #[test]
    fn auto_pin_unpin_some_first_sources() {
        let mut mixer = AudioMixer::<Option<i8>, u32>::new(Box::new(|v| *v), AudioMixerConfig { outputs: 2 });

        mixer.add_source(0, 100);
        assert_eq!(mixer.pop(), Some(super::AudioMixerOutput::SlotPinned(100, 0)));
        assert_eq!(mixer.pop(), Some(super::AudioMixerOutput::OutputSlotSrcChanged(0, Some(100))));
        assert_eq!(mixer.pop(), None);

        mixer.add_source(0, 101);
        assert_eq!(mixer.pop(), Some(super::AudioMixerOutput::SlotPinned(101, 1)));
        assert_eq!(mixer.pop(), Some(super::AudioMixerOutput::OutputSlotSrcChanged(1, Some(101))));
        assert_eq!(mixer.pop(), None);

        mixer.add_source(0, 102);
        assert_eq!(mixer.pop(), None);

        //now remove pinned source 100 then mixer must switched free slot to source 102
        mixer.remove_source(0, 100);
        assert_eq!(mixer.pop(), Some(super::AudioMixerOutput::SlotUnpinned(100, 0)));
        assert_eq!(mixer.pop(), Some(super::AudioMixerOutput::SlotPinned(102, 0)));
        assert_eq!(mixer.pop(), Some(super::AudioMixerOutput::OutputSlotSrcChanged(0, Some(102))));
        assert_eq!(mixer.pop(), None);

        mixer.add_source(0, 100);
        assert_eq!(mixer.pop(), None);
    }

    #[test]
    fn auto_set_silent_after_timeout() {
        let mut mixer = AudioMixer::<Option<i8>, u32>::new(Box::new(|v| *v), AudioMixerConfig { outputs: 1 });

        mixer.add_source(0, 100);
        mixer.add_source(0, 101);
        assert_eq!(mixer.pop(), Some(super::AudioMixerOutput::SlotPinned(100, 0)));
        assert_eq!(mixer.pop(), Some(super::AudioMixerOutput::OutputSlotSrcChanged(0, Some(100))));
        assert_eq!(mixer.pop(), None);

        mixer.push_pkt(100, 100, &Some(10));
        mixer.push_pkt(100, 101, &Some(10));
        assert_eq!(mixer.pop(), Some(super::AudioMixerOutput::OutputSlotPkt(0, Some(10))));
        assert_eq!(mixer.pop(), None);

        //will reset audio level to SILENT_LEVEL after AUDIO_SLOT_TIMEOUT_MS
        mixer.on_tick(100 + super::AUDIO_SLOT_TIMEOUT_MS);

        assert_eq!(mixer.sources.get(&100).expect("").audio_level(), super::SILENT_LEVEL);
        assert_eq!(mixer.output_slots[0].audio_level(), Some(super::SILENT_LEVEL));

        //now mixer will switch slot to source 101
        mixer.push_pkt(100 + super::AUDIO_SLOT_TIMEOUT_MS, 101, &Some(6));
        assert_eq!(mixer.pop(), Some(super::AudioMixerOutput::SlotUnpinned(100, 0)));
        assert_eq!(mixer.pop(), Some(super::AudioMixerOutput::SlotPinned(101, 0)));
        assert_eq!(mixer.pop(), Some(super::AudioMixerOutput::OutputSlotSrcChanged(0, Some(101))));
        assert_eq!(mixer.pop(), Some(super::AudioMixerOutput::OutputSlotPkt(0, Some(6))));
        assert_eq!(mixer.pop(), None);
    }

    #[test]
    fn switch_higher_source() {
        let mut mixer = AudioMixer::<Option<i8>, u32>::new(Box::new(|v| *v), AudioMixerConfig { outputs: 1 });

        mixer.add_source(0, 100);
        mixer.add_source(0, 101);
        assert_eq!(mixer.pop(), Some(super::AudioMixerOutput::SlotPinned(100, 0)));
        assert_eq!(mixer.pop(), Some(super::AudioMixerOutput::OutputSlotSrcChanged(0, Some(100))));
        assert_eq!(mixer.pop(), None);

        let first_level = 10;
        mixer.push_pkt(100, 100, &Some(first_level));
        mixer.push_pkt(100, 101, &Some(0));
        assert_eq!(mixer.pop(), Some(super::AudioMixerOutput::OutputSlotPkt(0, Some(first_level))));
        assert_eq!(mixer.pop(), None);

        //dont switch if dont higher than SWITCH_AUDIO_THRESHOLD
        mixer.push_pkt(100, 101, &Some(first_level + super::SWITCH_AUDIO_THRESHOLD / 2));
        assert_eq!(mixer.pop(), None);

        //now mixer will switch slot to source 101
        mixer.push_pkt(100, 101, &Some(first_level + super::SWITCH_AUDIO_THRESHOLD));
        assert_eq!(mixer.pop(), Some(super::AudioMixerOutput::SlotUnpinned(100, 0)));
        assert_eq!(mixer.pop(), Some(super::AudioMixerOutput::SlotPinned(101, 0)));
        assert_eq!(mixer.pop(), Some(super::AudioMixerOutput::OutputSlotSrcChanged(0, Some(101))));
        assert_eq!(mixer.pop(), Some(super::AudioMixerOutput::OutputSlotPkt(0, Some(first_level + super::SWITCH_AUDIO_THRESHOLD))));
        assert_eq!(mixer.pop(), None);
    }
}
