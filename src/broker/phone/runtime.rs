use super::Controller;
use super::resident;
use super::runtime_guard::RuntimeGuard;
use crate::broker::engine::Engine;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use teamy_cancellation::CancellationToken;
use tokio::time;

/// # Errors
/// Fails closed on invalid durable state; binds only the exact approved Tailscale endpoint.
pub async fn run(
    controller: Arc<Controller>,
    cancel: CancellationToken,
    engine: Arc<Engine>,
) -> eyre::Result<()> {
    eyre::ensure!(
        controller
            .actor_active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok(),
        "phone actor already running"
    );
    let _guard = RuntimeGuard::new(Arc::clone(&controller));
    for attempt in 0..3 {
        if cancel.is_cancelled() {
            return Ok(());
        }
        controller.runtime.store(1, Ordering::Release);
        if resident::run(Arc::clone(&controller), cancel.clone(), Arc::clone(&engine))
            .await
            .is_ok()
        {
            return Ok(());
        }
        controller.recovering();
        tracing::warn!(
            phase = "phone-actor-recovery",
            attempt,
            "No-vault phone actor failed; limited restart"
        );
        if attempt < 2 {
            time::sleep(Duration::from_secs(3 << attempt)).await;
        }
    }
    controller.runtime.store(3, Ordering::Release);
    eyre::bail!("phone actor failed; inspect phone-status")
}
