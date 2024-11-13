//! Audio mixing buffer implementation for combining multiple audio tracks
//!
//! This module provides functionality to mix multiple audio tracks into a single output.
//! The mixing process works with fixed-size frames of 20ms duration (960 samples).
//!
//! # Key concepts
//!
//! - Frames are processed in 20ms chunks (960 samples at 48kHz)
//! - Multiple tracks can be mixed into a single frame
//! - Frames are aligned based on timestamps with some tolerance
//! - Audio samples are mixed by adding their values and capping to i16 range
//!
//! # Constants
//!
//! - `FRAME_TS_DIFF_ACCEPTABLE`: Maximum allowed timestamp difference (100ms)
//! - `FRAME_QUEUE_LIMIT`: Maximum number of frames held in buffer (10 frames)

use std::{
    collections::{HashSet, VecDeque},
    hash::Hash,
};

const FRAME_TS_DIFF_ACCEPTABLE: u64 = 100;
const FRAME_QUEUE_LIMIT: usize = 10;

/// A single mixed audio frame containing samples from multiple tracks
///
/// # Type Parameters
///
/// * `TID` - Track identifier type that implements `Eq` and `Hash`
pub struct MixedFrame<TID> {
    /// Timestamp of the frame in milliseconds
    ts: u64,
    /// Buffer containing mixed audio samples
    buffer: [i32; 960],
    /// Set of track IDs that have contributed to this frame
    tracks: HashSet<TID>,
}

impl<TID: Eq + Hash> MixedFrame<TID> {
    fn put_into(&mut self, track_id: TID, ts: u64, data: &[i16]) -> bool {
        if data.len() > self.buffer.len() {
            log::warn!("[MixedFrame] input audio too big: {} vs {}", data.len(), self.buffer.len());
            return false;
        }
        if self.tracks.contains(&track_id) {
            return false;
        }
        // we reject the frame if the timestamp is too far from the frame timestamp
        if self.ts + FRAME_TS_DIFF_ACCEPTABLE <= ts || ts + FRAME_TS_DIFF_ACCEPTABLE <= self.ts {
            return false;
        }
        self.tracks.insert(track_id);
        for (i, sample) in data.iter().enumerate() {
            self.buffer[i] += *sample as i32;
        }
        true
    }
}

/// Buffer for mixing multiple audio tracks into a single output stream
///
/// Handles the queuing and mixing of audio frames from multiple sources,
/// maintaining temporal alignment using timestamps.
///
/// The buffer always keeps only `FRAME_QUEUE_LIMIT` frames for avoiding memory usage explosion.
///
/// # Type Parameters
///
/// * `TID` - Track identifier type that implements `Hash + Eq + Copy`
pub struct MixerBuffer<TID> {
    frames: VecDeque<MixedFrame<TID>>,
}

impl<TID: Hash + Eq + Copy> MixerBuffer<TID> {
    /// Creates a new empty mixer buffer
    pub fn new() -> Self {
        Self { frames: VecDeque::new() }
    }

    /// Pushes new audio data from a track into the buffer
    ///
    /// # Arguments
    ///
    /// * `ts` - Timestamp of the audio data in milliseconds
    /// * `track_id` - Identifier for the track
    /// * `data` - Audio samples as i16 values
    ///
    /// # Returns
    ///
    /// Returns `Some((timestamp, mixed_data))` if a frame was completed and popped,
    /// otherwise returns `None`
    pub fn push(&mut self, ts: u64, track_id: TID, data: &[i16]) -> Option<(u64, Vec<i16>)> {
        let frame_idx = self.push_data_to_frame(ts, track_id, data)?;
        log::debug!("push data to frame: {}", frame_idx);
        // we pop front frame if all tracks are mixed or the frame is too old
        if self.frames.len() > FRAME_QUEUE_LIMIT {
            self.force_pop()
        } else {
            None
        }
    }

    /// Forces the oldest frame to be popped and returned
    ///
    /// # Returns
    ///
    /// Returns `Some((timestamp, mixed_data))` if there was a frame to pop,
    /// otherwise returns `None`
    pub fn force_pop(&mut self) -> Option<(u64, Vec<i16>)> {
        self.frames.pop_front().map(|frame| (frame.ts, frame.buffer.iter().map(|s| cap_i16_cast(*s)).collect()))
    }

    /// Pushes new audio data from a track into the buffer
    ///
    /// # Returns
    ///
    /// Returns the index of the frame if the data was added, otherwise returns `None`
    fn push_data_to_frame(&mut self, ts: u64, track_id: TID, data: &[i16]) -> Option<usize> {
        for (i, frame) in self.frames.iter_mut().enumerate() {
            if frame.put_into(track_id, ts, data) {
                return Some(i);
            }
        }
        let front_ts = self.frames.front().map(|f| f.ts).unwrap_or(ts);
        // frame too old
        if ts < front_ts {
            return None;
        }
        let mut frame = MixedFrame {
            ts,
            buffer: [0; 960],
            tracks: HashSet::new(),
        };
        assert!(frame.put_into(track_id, ts, data), "should add data to new frame");
        self.frames.push_back(frame);
        Some(self.frames.len() - 1)
    }
}

impl<T> Drop for MixerBuffer<T> {
    fn drop(&mut self) {
        if !self.frames.is_empty() {
            log::warn!("drop audio mixer buffer with remaining {} frames", self.frames.len());
        }
    }
}

/// Caps an i32 value to fit within i16 range
///
/// # Arguments
///
/// * `v` - The i32 value to cap
///
/// # Returns
///
/// Returns the capped value as i16
fn cap_i16_cast(v: i32) -> i16 {
    if v > i16::MAX as i32 {
        i16::MAX
    } else if v < i16::MIN as i32 {
        i16::MIN
    } else {
        v as i16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_samples(value: i16) -> Vec<i16> {
        vec![value; 960]
    }

    #[test]
    fn test_mix_single_track() {
        let mut mixer = MixerBuffer::new();
        let samples = create_test_samples(100);

        // Push first frame
        let result = mixer.push(0, 1, &samples);
        assert!(result.is_none());

        // Push second frame to trigger force pop of first frame
        let samples2 = create_test_samples(200);
        let result = mixer.push(20, 1, &samples2);
        assert!(result.is_none());

        let (ts, data) = mixer.force_pop().expect("Should return mixed frame");
        assert_eq!(ts, 0);
        assert_eq!(data, create_test_samples(100));

        let (ts, data) = mixer.force_pop().expect("Should return mixed frame");
        assert_eq!(ts, 20);
        assert_eq!(data, create_test_samples(200));
    }

    #[test]
    fn test_mix_multi_tracks() {
        let mut mixer = MixerBuffer::new();

        // Push data from two tracks with same timestamp
        let samples1 = create_test_samples(100);
        let samples2 = create_test_samples(200);

        let result = mixer.push(0, 1, &samples1);
        assert!(result.is_none());

        let result = mixer.push(0, 2, &samples2);
        assert!(result.is_none());

        // Force pop the mixed frame
        let (ts, data) = mixer.force_pop().expect("Should return mixed frame");
        assert_eq!(ts, 0);
        assert_eq!(data, create_test_samples(300));
    }

    #[test]
    fn test_mix_single_track_with_gap() {
        let mut mixer = MixerBuffer::new();
        let samples = create_test_samples(100);

        // Push frame at t=0
        let result = mixer.push(0, 1, &samples);
        assert!(result.is_none());

        // Push frame at t=40 (skipping t=20)
        let result = mixer.push(40, 1, &samples);
        assert!(result.is_none());

        // Force pop should return the t=0 frame
        let (ts, data) = mixer.force_pop().expect("Should return mixed frame");
        assert_eq!(ts, 0);
        assert_eq!(data, create_test_samples(100));

        let (ts, data) = mixer.force_pop().expect("Should return mixed frame");
        assert_eq!(ts, 40);
        assert_eq!(data, create_test_samples(100));
    }

    #[test]
    fn test_mix_multi_tracks_with_slight_offset() {
        let mut mixer = MixerBuffer::new();
        let samples1 = create_test_samples(100);
        let samples2 = create_test_samples(200);

        // Push tracks with slight timestamp difference (within acceptable range)
        let result = mixer.push(0, 1, &samples1);
        assert!(result.is_none());

        let result = mixer.push(5, 2, &samples2);
        assert!(result.is_none());

        // Force pop should return mixed frame
        let (ts, data) = mixer.force_pop().expect("Should return mixed frame");
        assert_eq!(ts, 0);
        assert_eq!(data, create_test_samples(300));
    }

    #[test]
    fn test_frame_queue_limit() {
        let mut mixer = MixerBuffer::new();

        // Push FRAME_QUEUE_LIMIT + 1 frames
        for i in 0..=FRAME_QUEUE_LIMIT {
            let ts = (i * 20) as u64;
            let samples = create_test_samples(i as i16);
            let result = mixer.push(ts, 1, &samples);

            if i == FRAME_QUEUE_LIMIT {
                // Last push should trigger a pop
                assert_eq!(result, Some((0, create_test_samples(0))));
            } else {
                assert_eq!(result, None);
            }
        }
    }

    #[test]
    fn test_cap_i16_cast() {
        assert_eq!(cap_i16_cast(0), 0);
        assert_eq!(cap_i16_cast(i16::MAX as i32), i16::MAX);
        assert_eq!(cap_i16_cast(i16::MIN as i32), i16::MIN);
        assert_eq!(cap_i16_cast(i16::MAX as i32 + 1000), i16::MAX);
        assert_eq!(cap_i16_cast(i16::MIN as i32 - 1000), i16::MIN);
    }
}
