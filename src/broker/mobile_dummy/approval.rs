use crate::broker::approval::Approval;
use crate::broker::work_guard::WorkGuard;
use crate::broker::work_kind::WorkKind;
use crate::protocol::AcquireRequest;
use crate::protocol::FailureKind;
use crate::protocol::wire;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use teamy_broker_handoff::binding::Binding;
use teamy_broker_handoff::queue::Queue;
use tokio::task;
use tokio::time;
use tokio::time::Instant;
pub struct MobileDummyApproval {
    pub queue: Arc<Queue>,
}
impl Debug for MobileDummyApproval {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str("MobileDummyApproval([redacted])")
    }
}
impl Approval for MobileDummyApproval {
    fn approve<'a>(
        &'a self,
        request: &'a AcquireRequest,
        pid: u32,
        cancel: Arc<AtomicBool>,
        work: WorkGuard,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<bool>> + Send + 'a>> {
        approve_request(Arc::clone(&self.queue), request, pid, cancel, work, false)
    }
}
pub(super) fn approve_request<'a>(
    queue: Arc<Queue>,
    request: &'a AcquireRequest,
    pid: u32,
    cancel: Arc<AtomicBool>,
    work: WorkGuard,
    desktop: bool,
) -> Pin<Box<dyn Future<Output = eyre::Result<bool>> + Send + 'a>> {
    Box::pin(async move {
        let approval_work = work;
        eyre::ensure!(*request == super::request(), "dummy request required");
        let now = wire::unix_ms()?;
        let binding = Binding::dummy(pid, now)?;
        let (id, mut receiver) = queue.enqueue(binding.clone())?;
        if desktop {
            let state = super::desktop_prompt_state::DesktopPromptState {
                queue: Arc::clone(&queue),
                binding,
                work: approval_work.phase(WorkKind::Approval),
            };
            // Errors/dismissal of the optional desktop prompt do not cancel the phone request.
            let _desktop = task::spawn_blocking(move || {
                let _ = super::desktop_prompt::show(&state);
            });
        }
        let guard = super::pending_guard::PendingGuard {
            queue: Arc::clone(&queue),
            id,
        };
        // Poll cancel while waiting, preserving Engine's existing gate/work/version semantics.
        let deadline = Instant::now() + Duration::from_millis(teamy_broker_handoff::REQUEST_MS);
        let bytes = loop {
            if cancel.load(Ordering::SeqCst) {
                eyre::bail!("cancelled");
            }
            tokio::select! {
                result = &mut receiver => break result.map_err(|_redacted_error| eyre::Report::new(FailureKind::ApprovalTimedOut))?,
                () = time::sleep_until(deadline) => return Err(FailureKind::ApprovalTimedOut.into()),
                () = time::sleep(Duration::from_millis(20)) => {}
            }
        };
        drop(guard);
        if cancel.load(Ordering::SeqCst) {
            eyre::bail!("cancelled");
        }
        let approved = bytes
            .approved()
            .map_err(|_redacted_error| eyre::Report::new(FailureKind::ApprovalTimedOut))?;
        Ok(approved)
    })
}
