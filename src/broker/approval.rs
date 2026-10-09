use crate::protocol::AcquireRequest;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
pub trait Approval: Send + Sync {
    fn approve<'a>(
        &'a self,
        request: &'a AcquireRequest,
        pid: u32,
        cancel: Arc<AtomicBool>,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<bool>> + Send + 'a>>;
}
