use crate::constants::{CONG_C, CONG_C_SCALE};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CongestionState {
    Open = 1,
    Recovery = 2,
    Loss = 3,
}

#[derive(Debug, Clone)]
pub struct CongestionControl {
    k: u32,
    ack_cnt: u32,
    origin_point: u32,
    delay_min: u32,
    pub cnt: u32,
    last_time: std::time::Instant,
    start_time: std::time::Instant,
    last_max_cwnd: u32,
    last_cwnd: u32,
    tcp_cwnd: u32,
}

impl Default for CongestionControl {
    fn default() -> Self {
        Self::new()
    }
}

impl CongestionControl {
    pub fn new() -> Self {
        Self {
            k: 0,
            ack_cnt: 0,
            origin_point: 0,
            delay_min: 0,
            cnt: 0,
            last_time: std::time::Instant::now(),
            start_time: std::time::Instant::now(),
            last_max_cwnd: 0,
            last_cwnd: 0,
            tcp_cwnd: 0,
        }
    }

    pub fn update(&mut self, cwnd: u32, acked: u32, now: std::time::Instant) {
        self.ack_cnt += acked;

        if self.last_cwnd == cwnd && (now - self.last_time).as_millis() <= 3 {
            return;
        }

        if !self.start_time.elapsed().is_zero() {
            self.start_time = now;
            self.ack_cnt = acked;
            self.tcp_cwnd = cwnd;

            if self.last_max_cwnd <= cwnd {
                self.k = 0;
                self.origin_point = cwnd;
            } else {
                self.k = self.cubic_root(
                    CONG_C_SCALE / (CONG_C as u64) * (self.last_max_cwnd - cwnd) as u64,
                );
                self.origin_point = self.last_max_cwnd;
            }
        }

        let t = now.duration_since(self.start_time).as_millis() as u32 + self.delay_min;
        let d = if t < self.k { self.k - t } else { t - self.k };

        let delta = (CONG_C * d * d * d) as u64 / CONG_C_SCALE;

        let target = if t < self.k {
            self.origin_point - delta as u32
        } else {
            self.origin_point + delta as u32
        };

        self.cnt = if target > cwnd {
            cwnd / (target - cwnd)
        } else {
            100 * cwnd
        };

        if self.last_cwnd == 0 && self.cnt > 20 {
            self.cnt = 20;
        }
    }

    fn cubic_root(&self, a: u64) -> u32 {
        (a as f64).cbrt() as u32
    }
}
