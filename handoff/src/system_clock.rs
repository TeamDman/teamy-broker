use crate::clock::Clock;
use crate::clock_reading::ClockReading;
#[derive(Debug)]
pub struct SystemClock;
impl Clock for SystemClock {
    fn reading(&self) -> eyre::Result<ClockReading> {
        let unix_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|time| u64::try_from(time.as_millis()).ok())
            .ok_or_else(|| eyre::eyre!("invalid clock"))?;
        Ok(ClockReading {
            unix_ms,
            tick: std::time::Instant::now(),
        })
    }
}
