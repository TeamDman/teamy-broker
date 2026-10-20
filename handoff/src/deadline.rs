use crate::clock_reading::ClockReading;
use std::time::Instant;
#[derive(Clone, Copy, Debug)]
pub struct Deadline {
    pub expires_unix_ms: u64,
    pub tick: Instant,
}
impl Deadline {
    pub fn valid_at(self, reading: ClockReading) -> bool {
        reading.unix_ms < self.expires_unix_ms && reading.tick < self.tick
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn either_exact_boundary_rejects_without_sleep() {
        let start = Instant::now();
        let deadline = Deadline {
            expires_unix_ms: 2000,
            tick: start + std::time::Duration::from_secs(1),
        };
        assert!(deadline.valid_at(ClockReading {
            unix_ms: 1999,
            tick: start
        }));
        assert!(!deadline.valid_at(ClockReading {
            unix_ms: 2000,
            tick: start
        }));
        assert!(!deadline.valid_at(ClockReading {
            unix_ms: 1000,
            tick: deadline.tick
        }));
        assert!(!deadline.valid_at(ClockReading {
            unix_ms: 999,
            tick: deadline.tick + std::time::Duration::from_nanos(1)
        }));
    }
}
