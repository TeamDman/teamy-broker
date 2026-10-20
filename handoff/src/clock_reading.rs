use std::time::Instant;
#[derive(Clone, Copy, Debug)]
pub struct ClockReading {
    pub unix_ms: u64,
    pub tick: Instant,
}
