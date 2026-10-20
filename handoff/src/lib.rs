//! Experimental protocol. There is deliberately no production mode or persistent pairing.
pub mod binding;
mod entry;
pub mod envelope;
pub mod pending;
pub mod queue;
pub mod session;
pub mod wire;

pub const DUMMY_SECRET: &[u8] = b"TEAMY-DUMMY-ONLY";
pub const DESKTOP: &str = "Desktop (dummy session)";
pub const PHONE: &str = "Phone (dummy session)";
pub const CONSUMER: &str = "teamy-discord-dummy";
pub const REFERENCE: &str = "op://Synthetic/teamy-discord/token";
pub const PURPOSE: &str = "Dummy Discord authentication demonstration; no API call";
pub const LEASE_MS: u64 = 900_000;
pub const REQUEST_MS: u64 = 60_000;
pub const POLL_AAD: &[u8] = b"teamy-broker/dummy/poll/v1";
pub const SUBMIT_AAD: &[u8] = b"teamy-broker/dummy/submit/v1";

pub mod clock;
pub mod clock_reading;
pub mod deadline;
pub mod local_decision;
pub mod received_decision;
pub mod system_clock;

pub mod pairing_binding;

/// Build-local exact phone scope. An unconfigured build cannot activate the network listener.
pub mod phone_config {
    include!(concat!(env!("OUT_DIR"), "/phone_config.rs"));
}
