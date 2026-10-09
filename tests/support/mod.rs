pub mod fake_adapter;
pub mod fake_approval;
pub mod fake_storage;
use teamy_broker::protocol::AcquireRequest;
use teamy_broker::protocol::Backend;
pub fn request(backend: Backend) -> AcquireRequest {
    AcquireRequest {
        account: "synthetic-account".into(),
        secret_ref: "op://Synthetic/item/token".into(),
        purpose: "synthetic integration test".into(),
        requesting_app: "synthetic-consumer".into(),
        destination: "https://example.invalid".into(),
        backend,
    }
}
