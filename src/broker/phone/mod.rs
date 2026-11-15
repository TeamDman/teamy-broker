//! Explicitly enrolled phone relationship with TLS 1.3 mutual authentication.
use std::time::Duration;

pub mod config;
mod controller;
mod diagnostic;
mod identity;
mod message;
mod message_read;
pub mod pairing;
pub(crate) mod pairing_confirmation;
mod phone_listener;
mod preview_hub;
mod resident;
mod runtime;
mod runtime_guard;
mod store;
pub mod tailnet;
mod timing;
mod transport;

pub const ENROLLMENT_WINDOW: Duration = Duration::from_mins(5);
pub const SCOPE: &str = "persistent-no-vault-preview-v1";

pub use crate::protocol::PhoneStatus;
pub use controller::Controller;
pub use runtime::run;

#[cfg(test)]
mod faithful_integration;
#[cfg(test)]
mod tests;
