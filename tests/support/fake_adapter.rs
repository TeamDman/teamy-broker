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
            tokio::time::sleep(self.delay).await;
            eyre::ensure!(!self.fail, "synthetic adapter denial");
            Ok(Zeroizing::new(b"SYNTHETIC_SECRET_NEVER_LOG".to_vec()))
        })
    }
}
