use super::lease::Lease;
use super::storage::Storage;
#[derive(Debug)]
pub struct NativeStorage;
impl Storage for NativeStorage {
    fn publish(&self, lease: &Lease) -> eyre::Result<()> {
        super::persistence::publish(lease)
    }
    fn remove(&self, lease: &Lease) -> eyre::Result<()> {
        if lease.request.backend == crate::protocol::Backend::Memory {
            Ok(())
        } else {
            super::persistence::cleanup(&lease.id, true)
        }
    }
}
