const WARM_UP_BWE_BPS: u64 = 800_000; // in inatve or warm-up state we will used minimum WARM_UP_BWE_BPS
const WARM_UP_DESIRED_BPS: u64 = 1_000_000; // in inatve or warm-up state we will used minimum WARM_UP_DESIRED_BPS
const WARM_UP_MS: u128 = 5000;
const TIMEOUT_MS: u128 = 2000;

use std::time::Instant;

/// BweState manage stage of Bwe for avoiding video stream stuck or slow start.
///
/// - It start with Inactive state, in this state all bwe = bwe.max(DEFAULT_BWE_BPS)
/// - In WarmUp state, we reset bwe with WARM_UP_BWE_BPS, each time we receive bwe we rewrite bwe = bwe.max(DEFAULT_BWE_BPS). After WarmUp end it will be switched to Active
/// - In Active, dont change result. If after TIMEOUT_MS, we dont have video packet, it will be reset to Inactive
///
#[derive(Default, Debug, PartialEq, Eq)]
pub enum BweState {
    #[default]
    Inactive,
    WarmUp {
        started_at: Instant,
        last_video_pkt: Instant,
        reset_bwe: bool,
    },
    Active {
        last_video_pkt: Instant,
    },
}

impl BweState {
    /// Return Some(init_bitrate) if we need reset BWE
    pub fn on_tick(&mut self, now: Instant) -> Option<u64> {
        match self {
            Self::Inactive => None,
            Self::WarmUp {
                started_at,
                last_video_pkt,
                reset_bwe,
            } => {
                if *reset_bwe {
                    *reset_bwe = false;
                    log::info!("[BweState] reset to default BWE in WarmUp state");
                    Some(WARM_UP_BWE_BPS)
                } else if now.duration_since(*last_video_pkt).as_millis() >= TIMEOUT_MS {
                    log::info!("[BweState] switched from WarmUp to Inactive after {:?} not received video pkt", now.duration_since(*last_video_pkt));
                    *self = Self::Inactive;
                    Some(0)
                } else if now.duration_since(*started_at).as_millis() >= WARM_UP_MS {
                    log::info!("[BweState] switched from WarmUp to Active after {:?}", now.duration_since(*started_at));
                    *self = Self::Active { last_video_pkt: *last_video_pkt };
                    None
                } else {
                    None
                }
            }
            Self::Active { last_video_pkt } => {
                if now.duration_since(*last_video_pkt).as_millis() >= TIMEOUT_MS {
                    log::info!("[BweState] Switch to Inactive start after {TIMEOUT_MS} ms not send video data");
                    *self = Self::Inactive;
                    Some(0)
                } else {
                    None
                }
            }
        }
    }

    pub fn on_send_video(&mut self, now: Instant) {
        match self {
            Self::Inactive => {
                log::info!("[BweState] switched from Inactive to WarmUp with first video packet");
                *self = Self::WarmUp {
                    started_at: now,
                    last_video_pkt: now,
                    reset_bwe: true,
                }
            }
            Self::WarmUp { last_video_pkt, .. } | Self::Active { last_video_pkt } => {
                *last_video_pkt = now;
            }
        }
    }

    pub fn filter_bwe(&mut self, bwe: u64) -> u64 {
        match self {
            Self::Inactive => 0,
            Self::WarmUp { .. } => {
                let new_bwe = bwe.max(WARM_UP_BWE_BPS);
                log::debug!("[BweState] rewrite bwe {bwe} to {new_bwe} with WarmUp state");
                new_bwe
            }
            Self::Active { .. } => bwe,
        }
    }

    pub fn filter_bwe_config(&mut self, current: u64, desired: u64) -> (u64, u64) {
        match self {
            Self::Inactive => (0, 0),
            Self::WarmUp { .. } => {
                let new_c = current.max(WARM_UP_BWE_BPS);
                let new_d = desired.max(WARM_UP_DESIRED_BPS);
                log::debug!("[BweState] rewrite current {current}, desired {desired} to current {new_c}, desired {new_d} with WarmUp state",);
                (new_c, new_d)
            }
            Self::Active { .. } => (current, desired),
        }
    }
}

#[cfg(test)]
mod test {
    use std::time::{Duration, Instant};

    use crate::transport::bwe_state::{TIMEOUT_MS, WARM_UP_BWE_BPS, WARM_UP_DESIRED_BPS, WARM_UP_MS};

    use super::BweState;

    #[test]
    fn inactive_state() {
        let mut state = BweState::default();
        assert_eq!(state, BweState::Inactive);
        assert_eq!(state.on_tick(Instant::now()), None);

        assert_eq!(state.filter_bwe(100), 0);
        assert_eq!(state.filter_bwe_config(100, 200), (0, 0));
    }

    #[test]
    fn inactive_switch_to_warmup() {
        let mut state = BweState::default();

        let now = Instant::now();
        state.on_send_video(now);
        assert!(matches!(state, BweState::WarmUp { .. }));

        assert_eq!(state.filter_bwe(100), WARM_UP_BWE_BPS);
        assert_eq!(state.filter_bwe_config(100, 200), (WARM_UP_BWE_BPS, WARM_UP_DESIRED_BPS));

        assert_eq!(state.filter_bwe(WARM_UP_BWE_BPS + 100), WARM_UP_BWE_BPS + 100);
        assert_eq!(
            state.filter_bwe_config(WARM_UP_BWE_BPS + 100, WARM_UP_DESIRED_BPS + 200),
            (WARM_UP_BWE_BPS + 100, WARM_UP_DESIRED_BPS + 200)
        );
    }

    #[test]
    fn active_state() {
        let now = Instant::now();
        let mut state = BweState::Active { last_video_pkt: now };
        assert_eq!(state.filter_bwe(100), 100);
        assert_eq!(state.filter_bwe_config(100, 200), (100, 200));

        assert_eq!(state.on_tick(now), None);
        assert!(matches!(state, BweState::Active { .. }));

        // after timeout without video packet => reset to Inactive
        assert_eq!(state.on_tick(now + Duration::from_millis(TIMEOUT_MS as u64)), Some(0));
        assert!(matches!(state, BweState::Inactive));
    }

    #[test]
    fn warmup_auto_switch_active() {
        let now = Instant::now();
        let mut state = BweState::WarmUp {
            started_at: now,
            last_video_pkt: now,
            reset_bwe: true,
        };

        assert_eq!(state.on_tick(now), Some(WARM_UP_BWE_BPS));
        state.on_send_video(now + Duration::from_millis((WARM_UP_MS - 100) as u64));
        assert_eq!(state.on_tick(now + Duration::from_millis(WARM_UP_MS as u64)), None);
        assert!(matches!(state, BweState::Active { .. }));
    }

    #[test]
    fn warmup_auto_switch_inactive() {
        let now = Instant::now();
        let mut state = BweState::WarmUp {
            started_at: now,
            last_video_pkt: now,
            reset_bwe: true,
        };

        assert_eq!(state.on_tick(now), Some(WARM_UP_BWE_BPS));
        assert_eq!(state.on_tick(now + Duration::from_millis(TIMEOUT_MS as u64)), Some(0));
        assert!(matches!(state, BweState::Inactive));
    }
}
