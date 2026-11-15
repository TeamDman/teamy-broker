use crate::broker::adapter::Adapter;
use crate::broker::adapter::SecretRead;
use crate::protocol::AcquireRequest;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use zeroize::Zeroizing;
pub struct MobileDummyAdapter;
impl Debug for MobileDummyAdapter {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
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
