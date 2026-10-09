use super::approval::Approval;
use crate::protocol::AcquireRequest;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
#[derive(Debug)]
pub struct NativeApproval;
impl Approval for NativeApproval {
    fn approve<'a>(
        &'a self,
        request: &'a AcquireRequest,
        pid: u32,
        cancel: Arc<AtomicBool>,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<bool>> + Send + 'a>> {
        let request = request.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || {
                crate::platform::native_approval::approve(&request, pid, &cancel)
            })
            .await
            .map_err(|_redacted_error| eyre::eyre!("approval worker failed"))?
        })
    }
}
