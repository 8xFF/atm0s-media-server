#[derive(Clone)]
enum State {
    FirstInit,
    Reinit,
    Rewriting,
}

#[derive(Clone)]
pub struct TsRewrite<const TS_LIMIT: u64, const TS_DELTA_REINIT: u64> {
    data_rate: u64,
    delta_ts: i64,
    last_extended_ts: i64,
    last_rtp_ts: u64,
    state: State,
}

impl<const TS_LIMIT: u64, const TS_DELTA_REINIT: u64> TsRewrite<TS_LIMIT, TS_DELTA_REINIT> {
    pub fn new(data_rate: u64) -> Self {
        Self {
            data_rate,
            delta_ts: 0,
            last_extended_ts: 0,
            last_rtp_ts: 0,
            state: State::FirstInit,
        }
    }

    // mark that the stream is disconnected, the next is new stream then need to sync with new stream
    pub fn reinit(&mut self) {
        self.delta_ts = 0;
        self.state = State::Reinit
    }

    // generate new timestamp from input now_ms and rtp_ts
    pub fn generate(&mut self, now_ms: u64, rtp_ts: u64) -> u64 {
        match self.state {
            State::FirstInit => {
                let now_ts = now_ms as i64 * (self.data_rate as i64 / 1000);
                self.last_rtp_ts = rtp_ts;
                self.delta_ts = now_ts as i64 - rtp_ts as i64;
                self.state = State::Rewriting;
            }
            State::Reinit => {
                let mut now_ts = now_ms as i64 * (self.data_rate / 1000) as i64;
                if now_ts < self.last_extended_ts {
                    //why this happen?
                    now_ts = self.last_extended_ts + TS_DELTA_REINIT as i64;
                }
                self.last_rtp_ts = rtp_ts;
                self.delta_ts = now_ts - rtp_ts as i64;
                self.state = State::Rewriting;
            }
            State::Rewriting => {
                if self.last_rtp_ts as i64 + TS_LIMIT as i64 / 2 < rtp_ts as i64 {
                    //previous cycle
                    return ((self.delta_ts + rtp_ts as i64) % TS_LIMIT as i64) as u64;
                }

                self.last_rtp_ts = rtp_ts;
                if rtp_ts as i64 + TS_LIMIT as i64 / 2 < self.last_rtp_ts as i64 {
                    //next cycle
                    self.delta_ts += TS_LIMIT as i64;
                }
            }
        };

        self.last_extended_ts = self.delta_ts + rtp_ts as i64;

        (self.last_extended_ts % TS_LIMIT as i64) as u64
    }
}

#[cfg(test)]
mod test {

    enum Input {
        Reinit,
        Generate(u64, u64, u64),
    }

    fn test<const TS_LIMIT: u64, const TS_DELTA_REINIT: u64>(data_rate: u64, data: Vec<Input>) {
        let mut ts_rewrite = super::TsRewrite::<TS_LIMIT, TS_DELTA_REINIT>::new(data_rate);
        let mut index = 0;
        for input in data {
            index += 1;
            match input {
                Input::Reinit => {
                    ts_rewrite.reinit();
                }
                Input::Generate(now_ms, rtp_ts, expected) => {
                    let actual = ts_rewrite.generate(now_ms, rtp_ts);
                    assert_eq!(actual, expected, "wrong at row {}", index);
                }
            }
        }
    }

    #[test]
    fn normal_case() {
        test::<100000, 10>(
            1000,
            vec![
                Input::Generate(0, 0, 0),
                Input::Generate(200, 200, 200),
                Input::Generate(1000, 1000, 1000),
                Input::Generate(99999, 99999, 99999),
            ],
        );
    }

    #[test]
    fn reinit_case() {
        test::<100000, 10>(
            1000,
            vec![
                Input::Generate(0, 0, 0),
                Input::Generate(200, 200, 200),
                Input::Reinit,
                Input::Generate(1000, 210, 1000),
                Input::Generate(1200, 410, 1200),
            ],
        );
    }

    #[test]
    fn reinit_wait_case() {
        test::<100000, 10>(
            1000,
            vec![
                Input::Generate(0, 0, 0),
                Input::Generate(200, 200, 200),
                Input::Reinit,
                Input::Generate(1000, 510, 1000),
                Input::Generate(1200, 710, 1200),
            ],
        );
    }

    #[test]
    fn previous_cycle_case() {
        test::<100000, 10>(1000, vec![Input::Generate(99999, 99999, 99999), Input::Generate(1000, 200, 200)]);
    }

    #[test]
    fn next_cycle_case() {
        test::<100000, 10>(1000, vec![Input::Generate(99200, 99200, 99200), Input::Generate(99400, 99400, 99400), Input::Generate(100, 100, 100)]);
    }
}
