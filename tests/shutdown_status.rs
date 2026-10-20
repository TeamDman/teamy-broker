mod support;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use support::fake_adapter::FakeAdapter;
use support::fake_approval::FakeApproval;
use support::fake_storage::FakeStorage;
use support::request;
use teamy_broker::broker::engine::Engine;
use teamy_broker::protocol::Backend;
use teamy_broker::protocol::FailureKind;
use teamy_broker::protocol::stop_reason::StopReason;

async fn wait_for_calls(calls: &std::sync::atomic::AtomicUsize) {
    tokio::time::timeout(Duration::from_secs(2), async {
        while calls.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
async fn status(engine: &Engine) -> teamy_broker::protocol::Status {
    tokio::time::timeout(Duration::from_millis(200), engine.status())
        .await
        .unwrap()
        .unwrap()
}
fn operation(
    engine: &Arc<Engine>,
) -> tokio::task::JoinHandle<eyre::Result<teamy_broker::broker::grant::Grant>> {
    let engine = Arc::clone(engine);
    tokio::spawn(async move {
        engine
            .acquire(
                request(Backend::Memory),
                1,
                Arc::new(AtomicBool::new(false)),
            )
            .await
    })
}
#[tokio::test]
async fn idle_cached_lease_and_cleanup_reasons_are_distinct() {
    let storage = Arc::new(FakeStorage::default());
    let engine = Engine::new(
        Arc::new(FakeAdapter::default()),
        Arc::new(FakeApproval::default()),
        Arc::<FakeStorage>::clone(&storage),
    );
    assert!(status(&engine).await.safe_to_stop);
    let grant = engine
        .acquire(
            request(Backend::Memory),
            1,
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .unwrap();
    assert!(
        status(&engine).await.would_interrupt_operations,
        "grant handoff must remain tracked"
    );
    drop(grant);
    let idle = status(&engine).await;
    assert!(!idle.would_interrupt_operations);
    assert!(idle.would_discard_cached_leases);
    assert_eq!(idle.stop_reasons, [StopReason::CachedLeases]);
    assert_eq!(
        engine
            .stop(false)
            .await
            .unwrap_err()
            .downcast_ref::<FailureKind>(),
        Some(&FailureKind::ShutdownBlocked)
    );
    assert!(!engine.stopping.load(Ordering::SeqCst));
    storage.fail_remove.store(true, Ordering::SeqCst);
    let _ = engine.purge(None).await.unwrap_err();
    let cleanup = status(&engine).await;
    assert_eq!(cleanup.pending_cleanup, 1);
    assert_eq!(cleanup.active_leases, 0);
    assert_eq!(cleanup.stop_reasons, [StopReason::PendingCleanup]);
    assert_eq!(
        engine
            .stop(false)
            .await
            .unwrap_err()
            .downcast_ref::<FailureKind>(),
        Some(&FailureKind::ShutdownBlocked)
    );
    storage.fail_remove.store(false, Ordering::SeqCst);
    engine.expire().await.unwrap();
    assert!(
        status(&engine).await.safe_to_stop,
        "historical cleanup failures must not permanently block stop"
    );
    engine.stop(false).await.unwrap();
    assert_eq!(
        engine
            .acquire(
                request(Backend::Memory),
                1,
                Arc::new(AtomicBool::new(false))
            )
            .await
            .unwrap_err()
            .downcast_ref::<FailureKind>(),
        Some(&FailureKind::RequestCancelled)
    );
}
#[tokio::test]
async fn status_is_responsive_for_pending_open_and_queued_approvals_and_cancellation() {
    for dialog in [false, true] {
        let approval = Arc::new(FakeApproval {
            dialog,
            release: Some(Arc::new(tokio::sync::Notify::new())),
            ..Default::default()
        });
        let engine = Arc::new(Engine::new(
            Arc::new(FakeAdapter::default()),
            Arc::<FakeApproval>::clone(&approval),
            Arc::new(FakeStorage::default()),
        ));
        let stale = status(&engine).await;
        assert!(stale.safe_to_stop);
        let first = operation(&engine);
        wait_for_calls(&approval.calls).await;
        let second = operation(&engine);
        tokio::time::timeout(Duration::from_secs(2), async {
            while status(&engine).await.queued_requests != 1 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let busy = status(&engine).await;
        assert_eq!(busy.active_requests, 2);
        assert_eq!(busy.active_approvals, 1);
        assert_eq!(busy.open_approval_dialogs, u64::from(dialog));
        assert_eq!(busy.pending_approvals, u64::from(!dialog));
        assert_eq!(busy.upstream_reads, 0);
        assert!(!busy.safe_to_stop);
        assert_eq!(
            engine
                .stop(false)
                .await
                .unwrap_err()
                .downcast_ref::<FailureKind>(),
            Some(&FailureKind::ShutdownBlocked)
        );
        second.abort();
        first.abort();
        assert!(second.await.unwrap_err().is_cancelled());
        assert!(first.await.unwrap_err().is_cancelled());
        let idle = status(&engine).await;
        assert_eq!(idle.active_requests, 0);
        assert_eq!(idle.active_approvals, 0);
        assert_eq!(idle.open_approval_dialogs, 0);
        assert!(idle.safe_to_stop);
        engine.stop(false).await.unwrap();
    }
}
#[tokio::test]
async fn upstream_wait_is_not_reported_as_an_open_dialog_and_force_prevents_publication() {
    let release = Arc::new(tokio::sync::Notify::new());
    let adapter = Arc::new(FakeAdapter {
        release: Some(Arc::<tokio::sync::Notify>::clone(&release)),
        ..Default::default()
    });
    let storage = Arc::new(FakeStorage::default());
    let engine = Arc::new(Engine::new(
        Arc::<FakeAdapter>::clone(&adapter),
        Arc::new(FakeApproval::default()),
        Arc::<FakeStorage>::clone(&storage),
    ));
    let task = operation(&engine);
    wait_for_calls(&adapter.calls).await;
    let busy = status(&engine).await;
    assert_eq!(busy.upstream_reads, 1);
    assert_eq!(busy.active_approvals, 0);
    assert_eq!(busy.open_approval_dialogs, 0);
    assert!(busy.would_interrupt_operations);
    assert_eq!(
        engine
            .stop(false)
            .await
            .unwrap_err()
            .downcast_ref::<FailureKind>(),
        Some(&FailureKind::ShutdownBlocked)
    );
    engine.stop(true).await.unwrap();
    release.notify_one();
    let _ = task.await.unwrap().unwrap_err();
    assert!(storage.published.lock().unwrap().is_empty());
    assert_eq!(status(&engine).await.active_requests, 0);
}
