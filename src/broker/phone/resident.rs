use super::Controller;
use crate::broker::mobile_dummy::tailnet_peer::DESKTOP;
use crate::broker::mobile_dummy::tailnet_peer::PORT;
use crate::broker::mobile_dummy::tailnet_peer::TailnetPeer;
use crate::broker::mobile_dummy::{self};
use std::sync::Arc;
use std::sync::atomic::Ordering;
pub(super) async fn run(
    controller: Arc<Controller>,
    cancel: teamy_cancellation::CancellationToken,
    engine: Arc<crate::broker::engine::Engine>,
) -> eyre::Result<()> {
    let owned = Arc::clone(&controller);
    guarded(
        controller,
        run_inner(owned, cancel, engine, NetworkPolicy::Pinned),
    )
    .await
}
pub(super) async fn guarded(
    controller: Arc<Controller>,
    body: impl std::future::Future<Output = eyre::Result<()>>,
) -> eyre::Result<()> {
    let _guard = ListenerState(Arc::clone(&controller));
    body.await
}
#[derive(Debug)]
struct ListenerState(Arc<Controller>);
impl Drop for ListenerState {
    fn drop(&mut self) {
        self.0.cancel_pairing_comparison();
        self.0.listener.store(false, Ordering::Release);
        self.0.connected.store(false, Ordering::Release);
    }
}
pub(super) async fn run_inner(
    controller: Arc<Controller>,
    cancel: teamy_cancellation::CancellationToken,
    engine: Arc<crate::broker::engine::Engine>,
    policy: NetworkPolicy,
) -> eyre::Result<()> {
    let mut listener = None;
    let mut workers = tokio::task::JoinSet::new();
    let mut tick = tokio::time::interval(std::time::Duration::from_millis(250));
    let mut retry = tokio::time::Instant::now();
    let mut accept_failures = 0;
    loop {
        let state = controller.status()?;
        if cancel.is_cancelled() {
            break;
        }
        if !state.paired && !state.pairing_window_open {
            listener = None;
            controller.listener.store(false, Ordering::Release);
            workers.shutdown().await;
            controller.runtime.store(1, Ordering::Release);
        } else if listener.is_none() && tokio::time::Instant::now() >= retry {
            listener = policy.bind().await.ok();
            controller
                .listener
                .store(listener.is_some(), Ordering::Release);
            if listener.is_some() {
                controller.runtime.store(1, Ordering::Release);
            } else {
                controller.recovering();
            }
            retry = tokio::time::Instant::now() + std::time::Duration::from_secs(3);
        }
        tokio::select! {
            _=tick.tick()=>{},
            _=workers.join_next(),if !workers.is_empty()=>{},
            incoming=async{match &listener {Some(listener)=>listener.accept().await,None=>std::future::pending().await}}=>{
                let(socket,peer)=match incoming {
                    Ok(value)=>{accept_failures=0;value},
                    Err(_redacted_error)=>{
                        listener=None;controller.listener.store(false,Ordering::Release);controller.recovering();
                        workers.shutdown().await;accept_failures+=1;
                        eyre::ensure!(accept_failures<3,"phone listener failed");
                        retry=tokio::time::Instant::now()+std::time::Duration::from_secs(3);continue;
                    }
                };
                if !policy.permits_source(peer)||!workers.is_empty(){continue;}
                let controller=Arc::clone(&controller);let engine=Arc::clone(&engine);let policy=policy.clone();
                workers.spawn(async move {let attempt=super::diagnostic::Attempt::begin(Arc::clone(&controller));let success=dispatch(socket,peer,controller,engine,policy).await.is_ok();attempt.finish(success);});
            }
        }
    }
    workers.shutdown().await;
    controller.listener.store(false, Ordering::Release);
    Ok(())
}
async fn dispatch(
    socket: tokio::net::TcpStream,
    peer: std::net::SocketAddr,
    controller: Arc<Controller>,
    engine: Arc<crate::broker::engine::Engine>,
    policy: NetworkPolicy,
) -> eyre::Result<()> {
    let record = controller
        .record()?
        .ok_or_else(|| eyre::eyre!("phone not enabled"))?;
    if record.phone_certificate.is_some() {
        policy.verify_peer(peer).await?;
        return super::transport::connection(socket, peer, controller, record).await;
    }
    let (record, pair_until, attempt) = controller.capture_pairing()?;
    attempt.track(Arc::downgrade(&controller));
    #[cfg(test)]
    policy.attach(&attempt);
    let _guard = PairWindow(Arc::clone(&controller), Arc::clone(&attempt));
    controller.phase(crate::protocol::phone_status::PhonePhase::PeerVerification);
    policy.verify_peer(peer).await?;
    eyre::ensure!(
        controller.pairing_is_current(&record.generation, &attempt),
        "pairing attempt unavailable"
    );
    let _work = engine.control_work()?;
    let mut binding = teamy_broker_handoff::pairing_binding::PairingBinding::dummy(
        crate::protocol::wire::unix_ms()?,
    )?;
    binding.scope = super::SCOPE.into();
    let remaining = pair_until.saturating_duration_since(std::time::Instant::now());
    binding.pairing_expires_unix_ms =
        crate::protocol::wire::unix_ms()? + u64::try_from(remaining.as_millis())?;
    binding.session_expires_unix_ms = binding.pairing_expires_unix_ms;
    let until = tokio::time::Instant::now() + remaining;
    let java = mobile_dummy::java_pairing_worker::configured_java()?;
    let artifacts = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("pairing/java");
    controller.phase(crate::protocol::phone_status::PhonePhase::Pairing);
    let paired = policy
        .accept(socket, peer, (&java, &artifacts), &binding, until, attempt)
        .await?;
    #[cfg(test)]
    if policy.test_only() {
        policy.event("before_enrollment");
        drop(paired);
        policy.event("socket_dropped_before_enrollment");
        return Ok(());
    }
    controller.phase(crate::protocol::phone_status::PhonePhase::IdentityEnrollment);
    let generation = record.generation.clone();
    let certificate = record.certificate.clone();
    paired
        .enroll_preview_identity(&certificate, |phone| controller.enroll(&generation, phone))
        .await
}
#[derive(Debug)]
struct PairWindow(Arc<Controller>, Arc<super::pairing_confirmation::Attempt>);
impl Drop for PairWindow {
    fn drop(&mut self) {
        self.0.finish_pairing_attempt(&self.1);
    }
}

/// Sealed internal policy: production contains only exact pinned phone verification.
#[derive(Clone)]
pub(super) enum NetworkPolicy {
    Pinned,
    #[cfg(test)]
    Loopback {
        ready: Arc<std::sync::Mutex<Option<tokio::sync::oneshot::Sender<u16>>>>,
        events: Arc<super::faithful_integration::Events>,
    },
}
impl NetworkPolicy {
    async fn bind(&self) -> eyre::Result<tokio::net::TcpListener> {
        #[cfg(test)]
        if let Self::Loopback { ready, events } = self {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
            if let Some(sender) = ready
                .lock()
                .map_err(|_redacted_error| eyre::eyre!("test ready unavailable"))?
                .take()
            {
                let _ = sender.send(listener.local_addr()?.port());
            }
            events.record("TEAMY_TEST_ONLY_LOOPBACK_VERIFIER");
            return Ok(listener);
        }
        TailnetPeer::verify_desktop().await?;
        Ok(tokio::net::TcpListener::bind((DESKTOP, PORT)).await?)
    }
    fn permits_source(&self, peer: std::net::SocketAddr) -> bool {
        #[cfg(test)]
        if matches!(self, Self::Loopback { .. }) {
            return peer.ip().is_loopback();
        }
        match self {
            Self::Pinned => TailnetPeer::permits_source(peer),
            #[cfg(test)]
            Self::Loopback { .. } => peer.ip().is_loopback(),
        }
    }
    async fn verify_peer(&self, peer: std::net::SocketAddr) -> eyre::Result<()> {
        #[cfg(test)]
        if matches!(self, Self::Loopback { .. }) {
            eyre::ensure!(peer.ip().is_loopback(), "test peer rejected");
            return Ok(());
        }
        TailnetPeer::verify_peer(peer).await
    }
    async fn accept(
        &self,
        socket: tokio::net::TcpStream,
        peer: std::net::SocketAddr,
        runtime: (&std::path::Path, &std::path::Path),
        binding: &teamy_broker_handoff::pairing_binding::PairingBinding,
        until: tokio::time::Instant,
        attempt: Arc<super::pairing_confirmation::Attempt>,
    ) -> eyre::Result<mobile_dummy::paired_socket::PairedSocket> {
        let (java, artifacts) = runtime;
        #[cfg(test)]
        if self.test_only() {
            return mobile_dummy::pairing_server::accept_test_loopback_socket(
                socket, peer, java, artifacts, binding, until, attempt,
            )
            .await;
        }
        mobile_dummy::pairing_server::accept_persistent_socket(
            socket, peer, java, artifacts, binding, until, attempt,
        )
        .await
    }
    #[cfg(test)]
    fn test_only(&self) -> bool {
        matches!(self, Self::Loopback { .. })
    }
    #[cfg(test)]
    fn attach(&self, attempt: &Arc<super::pairing_confirmation::Attempt>) {
        if let Self::Loopback { events, .. } = self {
            attempt.attach_events(Arc::clone(events));
            events.record("accepted_socket_owner_one");
        }
    }
    #[cfg(test)]
    fn event(&self, event: &'static str) {
        if let Self::Loopback { events, .. } = self {
            events.record(event);
        }
    }
}
