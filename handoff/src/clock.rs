use crate::clock_reading::ClockReading;
pub trait Clock: Send + Sync {
    fn reading(&self) -> eyre::Result<ClockReading>;
}
