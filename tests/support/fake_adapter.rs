use std::pin::Pin;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;
use teamy_broker::broker::adapter::Adapter;
use teamy_broker::protocol::AcquireRequest;
use zeroize::Zeroizing;
#[derive(Debug, Default)]
pub struct FakeAdapter {
    pub calls: AtomicUsize,
    pub release: Option<std::sync::Arc<tokio::sync::Notify>>,
    pub fail: bool,
    pub delay: Duration,
}
impl Adapter for FakeAdapter {
    fn read<'a>(
        &'a self,
        _request: &'a AcquireRequest,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<Zeroizing<Vec<u8>>>> + Send + 'a>> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if let Some(release) = &self.release {
                release.notified().await;
            }
            tokio::time::sleep(self.delay).await;
            eyre::ensure!(!self.fail, "SYNTHETIC_UPSTREAM_DETAIL_SHOULD_NOT_LEAK");
            Ok(Zeroizing::new(b"SYNTHETIC_SECRET_NEVER_LOG".to_vec()))
        })
    }
}
