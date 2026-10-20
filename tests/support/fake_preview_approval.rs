use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use teamy_broker::broker::approval::Approval;
use teamy_broker::broker::work_guard::WorkGuard;
use teamy_broker::protocol::ApprovalPreviewDisposition;
use teamy_broker::protocol::ApprovalPreviewResult;
#[derive(Debug)]
pub struct FakePreviewApproval {
    pub calls: AtomicUsize,
    pub disposition: ApprovalPreviewDisposition,
    pub release: Option<Arc<tokio::sync::Notify>>,
    pub last_cancel: Mutex<Option<Arc<AtomicBool>>>,
}
impl FakePreviewApproval {
    pub fn new(disposition: ApprovalPreviewDisposition) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            disposition,
            release: None,
            last_cancel: Mutex::new(None),
        }
    }
}
impl Approval for FakePreviewApproval {
    fn approve<'a>(
        &'a self,
        _request: &'a teamy_broker::protocol::AcquireRequest,
        _pid: u32,
        _cancel: Arc<AtomicBool>,
        _work: WorkGuard,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<bool>> + Send + 'a>> {
        panic!("preview must never call credential approval")
    }
    fn preview<'a>(
        &'a self,
        pid: u32,
        cancel: Arc<AtomicBool>,
        work: WorkGuard,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<ApprovalPreviewResult>> + Send + 'a>> {
        Box::pin(async move {
            let _dialog = work.phase(teamy_broker::broker::work_kind::WorkKind::Dialog);
            let _work = work;
            *self.last_cancel.lock().unwrap() = Some(Arc::clone(&cancel));
            self.calls.fetch_add(1, Ordering::SeqCst);
            if let Some(release) = &self.release {
                release.notified().await;
            }
            Ok(ApprovalPreviewResult {
                disposition: if cancel.load(Ordering::SeqCst) {
                    ApprovalPreviewDisposition::Cancelled
                } else {
                    self.disposition
                },
                started_unix_ms: 1,
                finished_unix_ms: 2,
                broker_pid: std::process::id(),
                connected_client_pid: pid,
                broker_executable: "SYNTHETIC_PREVIEW_ONLY".into(),
                broker_sha256: None,
                task_dialog_hresult: 0,
                selected_button: 101,
                startup_flags: 1,
                startup_show_window: 0,
                presentation: None,
                attention: None,
                events: Vec::new(),
            })
        })
    }
}
