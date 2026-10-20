use std::time::Duration;
pub(super) const IDLE: Duration = Duration::from_secs(10);
pub(super) const FRAME: Duration = Duration::from_secs(5);
pub(super) const WRITE: Duration = Duration::from_secs(5);
#[cfg(test)]
pub(super) const CLIENT_CONNECT: Duration = Duration::from_secs(25);
#[cfg(test)]
pub(super) const CLIENT_EXCHANGE: Duration = Duration::from_secs(30);
