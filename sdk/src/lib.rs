//! Thin SDK: protocol + local IPC + lease-aware secret handoff. No op, dialogs, tracing subscriber, or CLI.
#[cfg(not(windows))]
compile_error!("teamy-broker V1 supports Windows only");
pub mod platform;
#[path = "../../src/protocol/mod.rs"]
pub mod protocol;
#[path = "../../src/sdk/mod.rs"]
mod sdk;
pub use sdk::AcquireRequest;
pub use sdk::Backend;
pub use sdk::Client;
pub use sdk::Secret;
pub use sdk::Status;
pub use sdk::resolve_provider;
