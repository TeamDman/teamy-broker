use crate::broker::lease::Lease;
use crate::broker::storage::Storage;
#[derive(Debug)]
pub struct DummyMemoryStorage;
impl Storage for DummyMemoryStorage {
    fn publish(&self, lease: &Lease) -> eyre::Result<()> {
        eyre::ensure!(
            lease.request == super::request()
                && lease.secret.as_slice() == teamy_broker_handoff::DUMMY_SECRET,
            "dummy memory grant required"
        );
        Ok(())
    }
    fn remove(&self, _lease: &Lease) -> eyre::Result<()> {
        Ok(())
    }
}
