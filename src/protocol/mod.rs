pub mod acquire_request;
pub mod approval_attention_outcome;
pub mod approval_preview_disposition;
pub mod approval_preview_event;
pub mod approval_preview_event_kind;
pub mod approval_preview_result;
pub mod credential_context;
pub mod shared_preview_snapshot;
pub use shared_preview_snapshot::SharedPreviewSnapshot;
pub use shared_preview_snapshot::SharedPreviewState;
pub const SHARED_PREVIEW_CAPABILITY: &str = "shared-no-vault-preview-v1";
pub mod backend;
pub mod hello;
pub mod pairing_comparison;
pub mod phone_status;
pub mod request;
pub mod response;
pub mod status;
pub use phone_status::PhoneStatus;
pub mod stop_reason;
pub mod wire;
pub use approval_attention_outcome::ApprovalAttentionOutcome;
pub use approval_preview_disposition::ApprovalPreviewDisposition;
pub use approval_preview_event::ApprovalPreviewEvent;
pub use approval_preview_event_kind::ApprovalPreviewEventKind;
pub use approval_preview_result::ApprovalPreviewResult;
pub const APPROVAL_PREVIEW_CAPABILITY: &str = "approval-preview-v2";
pub mod approval_presentation_outcome;
pub use acquire_request::AcquireRequest;
pub use approval_presentation_outcome::ApprovalPresentationOutcome;
pub use backend::Backend;
pub use hello::Hello;
pub use request::Request;
pub use response::Response;
pub use status::Status;
pub const VERSION: u32 = 2;
pub const LEASE_MILLIS: u64 = 900_000;
pub const MAX_SECRET: usize = 4096;
pub const CAPABILITIES: &[&str] = &[
    "typed-acquisition-errors-v1",
    "fixed-15-minute-lease",
    "memory",
    "windows-credential-manager",
    "well-known-file",
    "validate",
    "purge",
    "stop",
];

pub mod failure_kind;
pub use failure_kind::FailureKind;
