use crate::broker::phone::config::PhoneProfile;
use tokio::net::TcpListener;

/// A listener and the configuration that was verified before it was bound.
#[derive(Debug)]
pub(super) struct PhoneListener {
    pub socket: TcpListener,
    pub profile: Option<PhoneProfile>,
}
