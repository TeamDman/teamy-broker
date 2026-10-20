use crate::broker::adapter::Adapter;
use crate::broker::adapter::SecretRead;
use crate::protocol::AcquireRequest;
use zeroize::Zeroizing;
pub struct MobileDummyAdapter;
impl std::fmt::Debug for MobileDummyAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MobileDummyAdapter([redacted])")
    }
}
impl Adapter for MobileDummyAdapter {
    fn read<'a>(&'a self, request: &'a AcquireRequest) -> SecretRead<'a> {
        Box::pin(async move {
            eyre::ensure!(*request == super::request(), "dummy request required");
            Ok(Zeroizing::new(teamy_broker_handoff::DUMMY_SECRET.to_vec()))
        })
    }
}
