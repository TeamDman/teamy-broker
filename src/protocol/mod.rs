pub mod acquire_request;
pub mod backend;
pub mod hello;
pub mod request;
pub mod response;
pub mod status;
pub mod wire;
pub use acquire_request::AcquireRequest;
pub use backend::Backend;
pub use hello::Hello;
pub use request::Request;
pub use response::Response;
pub use status::Status;
pub const VERSION: u32 = 1;
pub const LEASE_MILLIS: u64 = 900_000;
pub const MAX_SECRET: usize = 4096;
pub const CAPABILITIES: &[&str] = &[
    "fixed-15-minute-lease",
    "memory",
    "windows-credential-manager",
    "well-known-file",
    "validate",
    "purge",
    "stop",
];
