use crate::protocol::AcquireRequest;
use std::pin::Pin;
use zeroize::Zeroizing;
pub type SecretRead<'a> =
    Pin<Box<dyn Future<Output = eyre::Result<Zeroizing<Vec<u8>>>> + Send + 'a>>;
pub trait Adapter: Send + Sync {
    fn read<'a>(&'a self, request: &'a AcquireRequest) -> SecretRead<'a>;
}
