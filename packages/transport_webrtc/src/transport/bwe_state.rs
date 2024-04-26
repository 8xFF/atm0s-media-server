const DEFAULT_BWE_BPS: u64 = 800_000; // in inatve or warm-up state we will used minimum DEFAULT_BWE_BPS
const DEFAULT_DESIRED_BPS: u64 = 1_000_000; // in inatve or warm-up state we will used minimum DEFAULT_DESIRED_BPS
const WARM_UP_FIRST_STAGE_MS: u128 = 1000;
const WARM_UP_MS: u128 = 2000;
const TIMEOUT_MS: u128 = 2000;
const MAX_BITRATE_BPS: u64 = 3_000_000;

use std::time::Instant;

/// BweState manage stage of Bwe for avoiding video stream stuck or slow start.
///
/// - It start with Inactive state, in this state all bwe = bwe.max(DEFAULT_BWE_BPS)
/// - In WarmUp state, it have 2 phase, each phase is 1 seconds.
/// After first phase, the Bwe will be reset with lastest_bwe.max(DEFAULT_BWE_BPS).
/// In this phase, bwe = bwe.max(DEFAULT_BWE_BPS). After WarmUp end it will be switched to Active
/// - In Active, bwe = bwe.min(MAX_BITRATE_BPS). If after TIMEOUT_MS, we dont have video packet, it will be reset to Inactive
///
/// In all state, bwe will have threshold MAX_BITRATE_BPS
///
#[derive(Default, Debug, PartialEq, Eq)]
pub enum BweState {
    #[default]
    Inactive,
    WarmUp {
        started_at: Instant,
        last_video_pkt: Instant,
        first_stage: bool,
        last_bwe: Option<u64>,
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
                first_stage,
                last_bwe,
            } => {
                if now.duration_since(*last_video_pkt).as_millis() >= TIMEOUT_MS {
                    log::info!("[BweState] switched from WarmUp to Inactive after {:?} not received video pkt", now.duration_since(*last_video_pkt));
                    *self = Self::Inactive;
                    return None;
                } else if now.duration_since(*started_at).as_millis() >= WARM_UP_MS {
                    log::info!("[BweState] switched from WarmUp to Active after {:?}", now.duration_since(*started_at));
                    *self = Self::Active { last_video_pkt: *last_video_pkt };
                    None
                } else if *first_stage && now.duration_since(*started_at).as_millis() >= WARM_UP_FIRST_STAGE_MS {
                    let init_bitrate = last_bwe.unwrap_or(DEFAULT_BWE_BPS).max(DEFAULT_BWE_BPS);
                    log::info!("[BweState] WarmUp first_stage end after {:?} => reset Bwe({init_bitrate})", now.duration_since(*started_at));
                    *first_stage = false;
                    Some(init_bitrate)
                } else {
                    None
                }
            }
            Self::Active { last_video_pkt } => {
                if now.duration_since(*last_video_pkt).as_millis() >= TIMEOUT_MS {
                    *self = Self::Inactive;
                }
                None
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
                    first_stage: true,
                    last_bwe: None,
                }
            }
            Self::WarmUp { last_video_pkt, .. } | Self::Active { last_video_pkt } => {
                *last_video_pkt = now;
            }
        }
    }

    pub fn filter_bwe(&mut self, bwe: u64) -> u64 {
        match self {
            Self::Inactive => {
                log::debug!("[BweState] rewrite bwe {bwe} to {} with Inactive or WarmUp state", bwe.max(DEFAULT_BWE_BPS));
                bwe.max(DEFAULT_BWE_BPS).min(MAX_BITRATE_BPS)
            }
            Self::WarmUp { last_bwe, .. } => {
                log::debug!("[BweState] rewrite bwe {bwe} to {} with Inactive or WarmUp state", bwe.max(DEFAULT_BWE_BPS));
                *last_bwe = Some(bwe);
                bwe.max(DEFAULT_BWE_BPS).min(MAX_BITRATE_BPS)
            }
            Self::Active { .. } => bwe.min(MAX_BITRATE_BPS),
        }
    }

    pub fn filter_bwe_config(&mut self, current: u64, desired: u64) -> (u64, u64) {
        match self {
            Self::Inactive | Self::WarmUp { .. } => {
                log::debug!(
                    "[BweState] rewrite current {current}, desired {desired} to current {}, desired {} with Inactive or WarmUp state",
                    current.max(DEFAULT_BWE_BPS),
                    desired.max(DEFAULT_DESIRED_BPS)
                );
                (current.max(DEFAULT_BWE_BPS).min(MAX_BITRATE_BPS), desired.max(DEFAULT_DESIRED_BPS).min(MAX_BITRATE_BPS))
            }
            Self::Active { .. } => (current.min(MAX_BITRATE_BPS), desired.min(MAX_BITRATE_BPS)),
        }
    }
}

#[cfg(test)]
mod test {
    use std::time::{Duration, Instant};

    use crate::transport::bwe_state::{DEFAULT_BWE_BPS, DEFAULT_DESIRED_BPS, TIMEOUT_MS, WARM_UP_FIRST_STAGE_MS, WARM_UP_MS};

    use super::BweState;

    #[test]
    fn inactive_state() {
        let mut state = BweState::default();
        assert_eq!(state, BweState::Inactive);
        assert_eq!(state.on_tick(Instant::now()), None);

        assert_eq!(state.filter_bwe(100), DEFAULT_BWE_BPS);
        assert_eq!(state.filter_bwe_config(100, 200), (DEFAULT_BWE_BPS, DEFAULT_DESIRED_BPS));
    }

    #[test]
    fn inactive_switch_to_warmup() {
        let mut state = BweState::default();

        let now = Instant::now();
        state.on_send_video(now);
        assert!(matches!(state, BweState::WarmUp { .. }));

        assert_eq!(state.filter_bwe(100), DEFAULT_BWE_BPS);
        assert_eq!(state.filter_bwe_config(100, 200), (DEFAULT_BWE_BPS, DEFAULT_DESIRED_BPS));

        assert_eq!(state.filter_bwe(DEFAULT_BWE_BPS + 100), DEFAULT_BWE_BPS + 100);
        assert_eq!(
            state.filter_bwe_config(DEFAULT_BWE_BPS + 100, DEFAULT_DESIRED_BPS + 200),
            (DEFAULT_BWE_BPS + 100, DEFAULT_DESIRED_BPS + 200)
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
        assert_eq!(state.on_tick(now + Duration::from_millis(TIMEOUT_MS as u64)), None);
        assert!(matches!(state, BweState::Inactive));
    }

    #[test]
    fn warmup_auto_switch_active() {
        let now = Instant::now();
        let mut state = BweState::WarmUp {
            started_at: now,
            last_video_pkt: now,
            first_stage: true,
            last_bwe: None,
        };

        assert_eq!(state.on_tick(now), None);
        assert_eq!(state.on_tick(now + Duration::from_millis(WARM_UP_FIRST_STAGE_MS as u64)), Some(DEFAULT_BWE_BPS));

        state.on_send_video(now + Duration::from_millis(100));

        assert_eq!(state.on_tick(now + Duration::from_millis(WARM_UP_MS as u64)), None);
        assert!(matches!(state, BweState::Active { .. }));
    }

    #[test]
    fn warmup_auto_switch_inactive() {
        let now = Instant::now();
        let mut state = BweState::WarmUp {
            started_at: now,
            last_video_pkt: now,
            first_stage: true,
            last_bwe: None,
        };

        assert_eq!(state.on_tick(now), None);
        assert_eq!(state.on_tick(now + Duration::from_millis(WARM_UP_FIRST_STAGE_MS as u64)), Some(DEFAULT_BWE_BPS));

        assert_eq!(state.on_tick(now + Duration::from_millis(TIMEOUT_MS as u64)), None);
        assert!(matches!(state, BweState::Inactive));
    }
}
