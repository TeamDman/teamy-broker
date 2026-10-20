use std::collections::VecDeque;
use std::sync::Mutex;
use teamy_broker_handoff::clock::Clock;
use teamy_broker_handoff::clock_reading::ClockReading;
pub struct TestClock {
    current: Mutex<ClockReading>,
    script: Mutex<VecDeque<ClockReading>>,
}
impl TestClock {
    pub fn new(unix_ms: u64) -> Self {
        Self {
            current: Mutex::new(ClockReading {
                unix_ms,
                tick: std::time::Instant::now(),
            }),
            script: Mutex::new(VecDeque::new()),
        }
    }
    pub fn set(&self, reading: ClockReading) {
        *self.current.lock().unwrap() = reading;
    }
    pub fn script(&self, readings: &[ClockReading]) {
        *self.script.lock().unwrap() = readings.iter().copied().collect();
    }
}
impl Clock for TestClock {
    fn reading(&self) -> eyre::Result<ClockReading> {
        if let Some(reading) = self.script.lock().unwrap().pop_front() {
            self.set(reading);
            Ok(reading)
        } else {
            Ok(*self.current.lock().unwrap())
        }
    }
}
