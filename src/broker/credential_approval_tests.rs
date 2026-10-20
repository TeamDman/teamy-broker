use super::shared_preview::SharedPreviewHub;
use crate::protocol::AcquireRequest;
use crate::protocol::ApprovalPreviewDisposition as D;
use crate::protocol::Backend;
use crate::protocol::credential_context::CredentialContext;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use zeroize::Zeroizing;
struct Guard(Arc<AtomicBool>);
impl Drop for Guard {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}
fn request() -> AcquireRequest {
    AcquireRequest {
        account: "Synthetic".into(),
        secret_ref: "op://Synthetic/Example/token".into(),
        requesting_app: "synthetic-discord".into(),
        purpose: "read-only dummy messages".into(),
        destination: "https://discord.invalid/channels/dummy".into(),
        backend: Backend::Memory,
    }
}
fn context(supply: bool) -> CredentialContext {
    CredentialContext {
        request: request(),
        supply,
        connected_process_id: 1,
        provider_state: "idle".into(),
        synthetic_fixture: false,
    }
}
#[test]
fn per_action_permission_exact_metadata_memory_only_and_expiry() {
    let p = super::credential_permission::CredentialPermission::new(request(), true).unwrap();
    assert!(p.matches(&request()));
    for field in 0..5 {
        let mut other = request();
        match field {
            0 => other.account.push('x'),
            1 => other.secret_ref.push('x'),
            2 => other.requesting_app.push('x'),
            3 => other.purpose.push('x'),
            _ => other.destination.push('x'),
        }
        assert!(!p.matches(&other));
    }
    let mut disk = request();
    disk.backend = Backend::WellKnownFile("synthetic".into());
    let _ = super::credential_permission::CredentialPermission::new(disk, true).unwrap_err();
}
#[test]
fn provider_authorization_is_intermediate_and_supply_first_wins() {
    let hub = SharedPreviewHub::default();
    let shared = hub
        .begin_credential(Arc::new(AtomicBool::new(false)), context(true))
        .unwrap();
    assert!(shared.request_provider());
    assert!(!shared.request_provider());
    assert!(shared.outcome().is_none());
    assert_eq!(
        hub.snapshot().unwrap().credential.unwrap().provider_state,
        "waiting"
    );
    assert!(shared.supply(Zeroizing::new(b"synthetic-phone".to_vec())));
    assert!(!shared.provider_value(Zeroizing::new(b"synthetic-late-provider".to_vec())));
    assert_eq!(&*shared.take_supply().unwrap(), b"synthetic-phone");
    assert!(shared.take_supply().is_none());
    assert!(hub.snapshot().is_none());
    assert!(!format!("{shared:?}").contains("synthetic-phone"));
}
#[test]
fn provider_first_rejects_late_supply_and_disabled_supply_never_accepts() {
    let hub = SharedPreviewHub::default();
    let shared = hub
        .begin_credential(Arc::new(AtomicBool::new(false)), context(false))
        .unwrap();
    assert!(!shared.supply(Zeroizing::new(b"synthetic".to_vec())));
    assert!(shared.request_provider());
    assert!(shared.provider_value(Zeroizing::new(b"synthetic-provider".to_vec())));
    assert!(!shared.supply(Zeroizing::new(b"late".to_vec())));
    assert_eq!(&*shared.take_supply().unwrap(), b"synthetic-provider");
}
#[test]
fn deny_cancel_expiry_and_stale_ids_prohibit_values() {
    for disposition in [D::DeniedTest, D::Cancelled, D::TimedOut, D::Unavailable] {
        let hub = SharedPreviewHub::default();
        let shared = hub
            .begin_credential(Arc::new(AtomicBool::new(false)), context(true))
            .unwrap();
        assert!(shared.resolve(disposition));
        assert!(!shared.supply(Zeroizing::new(b"synthetic".to_vec())));
        assert!(!shared.provider_value(Zeroizing::new(b"late".to_vec())));
        assert!(shared.take_supply().is_none());
    }
    let hub = SharedPreviewHub::default();
    let cancel = Arc::new(AtomicBool::new(false));
    let shared = hub
        .begin_credential(Arc::clone(&cancel), context(true))
        .unwrap();
    cancel.store(true, Ordering::SeqCst);
    assert!(!shared.supply(Zeroizing::new(b"late".to_vec())));
    assert_eq!(shared.outcome(), Some(D::Cancelled));
    assert!(!hub.supply(
        &uuid::Uuid::new_v4().to_string(),
        Zeroizing::new(b"stale".to_vec())
    ));
}
#[tokio::test]
async fn user_supply_cancels_pending_provider_future_and_keeps_request_context() {
    let hub = SharedPreviewHub::default();
    let shared = hub
        .begin_credential(Arc::new(AtomicBool::new(false)), context(true))
        .unwrap();
    shared.request_provider();
    let started = Arc::new(AtomicBool::new(false));
    let dropped = Arc::new(AtomicBool::new(false));
    let s = Arc::clone(&started);
    let d = Arc::clone(&dropped);
    let pending = Arc::clone(&shared);
    let actor = tokio::spawn(async move {
        super::credential_approvals::resolve(pending, async move {
            let _guard = Guard(d);
            s.store(true, Ordering::SeqCst);
            std::future::pending::<eyre::Result<Zeroizing<Vec<u8>>>>().await
        })
        .await
    });
    while !started.load(Ordering::SeqCst) {
        tokio::task::yield_now().await;
    }
    assert!(hub.snapshot().unwrap().credential.is_some());
    assert!(shared.supply(Zeroizing::new(b"synthetic-user".to_vec())));
    assert_eq!(
        &*tokio::time::timeout(std::time::Duration::from_secs(1), actor)
            .await
            .unwrap()
            .unwrap()
            .unwrap(),
        b"synthetic-user"
    );
    assert!(dropped.load(Ordering::SeqCst));
}
#[tokio::test]
async fn provider_failure_keeps_supply_available_without_duplicate_provider() {
    let hub = SharedPreviewHub::default();
    let shared = hub
        .begin_credential(Arc::new(AtomicBool::new(false)), context(true))
        .unwrap();
    shared.request_provider();
    let actor_shared = Arc::clone(&shared);
    let actor = tokio::spawn(async move {
        super::credential_approvals::resolve(actor_shared, async {
            Err(eyre::eyre!("synthetic provider unavailable"))
        })
        .await
    });
    while !shared.provider_failed.load(Ordering::Acquire) {
        tokio::task::yield_now().await;
    }
    assert!(shared.outcome().is_none());
    assert!(!shared.request_provider());
    assert!(shared.supply(Zeroizing::new(b"synthetic-fallback".to_vec())));
    actor.await.unwrap().unwrap();
}
#[test]
fn no_vault_snapshot_shape_unchanged_and_credential_snapshot_contains_no_value() {
    let hub = SharedPreviewHub::default();
    let preview = hub.begin(Arc::new(AtomicBool::new(false))).unwrap();
    let json = facet_json::to_string(&hub.snapshot().unwrap()).unwrap();
    assert!(!json.contains("credential"));
    preview.resolve(D::DeniedTest);
    let credential = hub
        .begin_credential(Arc::new(AtomicBool::new(false)), context(true))
        .unwrap();
    assert!(credential.supply(Zeroizing::new(b"never-in-metadata".to_vec())));
    assert!(!format!("{credential:?}").contains("never-in-metadata"));
}
