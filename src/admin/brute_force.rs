use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

const MAX_FAILED_ATTEMPTS: u32 = 5;
const BLOCK_DURATION: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone)]
struct LoginAttemptInfo {
    failed_count: u32,
    last_failed_at: Instant,
}

pub struct LoginTracker {
    attempts: HashMap<IpAddr, LoginAttemptInfo>,
}

impl LoginTracker {
    pub fn new() -> Self {
        LoginTracker {
            attempts: HashMap::new(),
        }
    }

    pub fn is_blocked(&self, ip: &IpAddr) -> bool {
        if let Some(info) = self.attempts.get(ip) {
            if info.failed_count >= MAX_FAILED_ATTEMPTS {
                return info.last_failed_at.elapsed() < BLOCK_DURATION;
            }
        }
        false
    }

    pub fn record_failure(&mut self, ip: IpAddr) {
        let info = self.attempts.entry(ip).or_insert(LoginAttemptInfo {
            failed_count: 0,
            last_failed_at: Instant::now(),
        });

        // Reset if block duration has passed
        if info.failed_count >= MAX_FAILED_ATTEMPTS
            && info.last_failed_at.elapsed() >= BLOCK_DURATION
        {
            info.failed_count = 0;
        }

        info.failed_count += 1;
        info.last_failed_at = Instant::now();
    }

    pub fn record_success(&mut self, ip: &IpAddr) {
        self.attempts.remove(ip);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn test_ip() -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))
    }

    #[test]
    fn test_not_blocked_initially() {
        let tracker = LoginTracker::new();
        assert!(!tracker.is_blocked(&test_ip()));
    }

    #[test]
    fn test_blocked_after_5_failures() {
        let mut tracker = LoginTracker::new();
        let ip = test_ip();
        for _ in 0..5 {
            tracker.record_failure(ip);
        }
        assert!(tracker.is_blocked(&ip));
    }

    #[test]
    fn test_not_blocked_after_4_failures() {
        let mut tracker = LoginTracker::new();
        let ip = test_ip();
        for _ in 0..4 {
            tracker.record_failure(ip);
        }
        assert!(!tracker.is_blocked(&ip));
    }

    #[test]
    fn test_reset_on_success() {
        let mut tracker = LoginTracker::new();
        let ip = test_ip();
        for _ in 0..5 {
            tracker.record_failure(ip);
        }
        assert!(tracker.is_blocked(&ip));
        tracker.record_success(&ip);
        assert!(!tracker.is_blocked(&ip));
    }
}
