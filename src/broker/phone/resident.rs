use super::Controller;
#[cfg(test)]
use super::config::JavaExecutablePath;
use super::pairing;
use super::pairing::pairing_runtime::PairingRuntime;
use super::phone_listener::PhoneListener;
use crate::broker::engine::Engine;
use crate::broker::phone::config::PhoneProfile;
use crate::broker::phone::tailnet::PORT;
use crate::broker::phone::tailnet::TailnetPeer;
use crate::paths::AppHome;
use crate::protocol::phone_status::PhonePhase;
use crate::protocol::wire;
use std::future;
use std::net::SocketAddr;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
#[cfg(test)]
use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant as StdInstant;
use teamy_broker_handoff::pairing_binding::PairingBinding;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
#[cfg(test)]
use tokio::sync::oneshot::Sender;
use tokio::task::JoinSet;
use tokio::time;
use tokio::time::Instant;
pub(super) async fn run(
    controller: Arc<Controller>,
    cancel: teamy_cancellation::CancellationToken,
    engine: Arc<Engine>,
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
    body: impl Future<Output = eyre::Result<()>>,
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
    engine: Arc<Engine>,
    policy: NetworkPolicy,
) -> eyre::Result<()> {
    let mut listener: Option<PhoneListener> = None;
    let mut workers = JoinSet::new();
    let mut tick = time::interval(Duration::from_millis(250));
    let mut retry = Instant::now();
    let mut accept_failures = 0;
    loop {
        let state = controller.status()?;
        if cancel.is_cancelled() {
            break;
        }
        if listener
            .as_ref()
            .and_then(|bound| bound.profile.as_ref())
            .is_some_and(|profile| TailnetPeer::require_current(profile).is_err())
        {
            listener = None;
            controller.listener.store(false, Ordering::Release);
            workers.shutdown().await;
        }
        if !state.paired && !state.pairing_window_open {
            listener = None;
            controller.listener.store(false, Ordering::Release);
            workers.shutdown().await;
            controller.runtime.store(1, Ordering::Release);
        } else if listener.is_none() && Instant::now() >= retry {
            listener = policy.bind().await.ok();
            controller
                .listener
                .store(listener.is_some(), Ordering::Release);
            if listener.is_some() {
                controller.runtime.store(1, Ordering::Release);
            } else {
                controller.recovering();
            }
            retry = Instant::now() + Duration::from_secs(3);
        }
        tokio::select! {
            _=tick.tick()=>{},
            _=workers.join_next(),if !workers.is_empty()=>{},
            incoming=async{match &listener {Some(listener)=>listener.socket.accept().await,None=>future::pending().await}}=>{
                let(socket,peer)=match incoming {
                    Ok(value)=>{accept_failures=0;value},
                    Err(_redacted_error)=>{
                        listener=None;controller.listener.store(false,Ordering::Release);controller.recovering();
                        workers.shutdown().await;accept_failures+=1;
                        eyre::ensure!(accept_failures<3,"phone listener failed");
                        retry=Instant::now()+Duration::from_secs(3);continue;
                    }
                };
                let profile=listener.as_ref().and_then(|bound| bound.profile.clone());
                if !policy.permits_source(peer, profile.as_ref())||!workers.is_empty(){continue;}
                let controller=Arc::clone(&controller);let engine=Arc::clone(&engine);let policy=policy.clone();
                workers.spawn(async move {let attempt=super::diagnostic::Attempt::begin(Arc::clone(&controller));let success=dispatch(socket,peer,controller,engine,policy,profile).await.is_ok();attempt.finish(success);});
            }
        }
    }
    workers.shutdown().await;
    controller.listener.store(false, Ordering::Release);
    Ok(())
}
async fn dispatch(
    socket: TcpStream,
    peer: SocketAddr,
    controller: Arc<Controller>,
    engine: Arc<Engine>,
    policy: NetworkPolicy,
    profile: Option<PhoneProfile>,
) -> eyre::Result<()> {
    let record = controller
        .record()?
        .ok_or_else(|| eyre::eyre!("phone not enabled"))?;
    if record.phone_certificate.is_some() {
        policy.verify_peer(peer, profile.as_ref()).await?;
        return super::transport::connection(
            socket,
            peer,
            controller,
            record,
            profile.ok_or_else(|| eyre::eyre!("phone profile unavailable"))?,
        )
        .await;
    }
    let (record, pair_until, attempt) = controller.capture_pairing()?;
    attempt.track(Arc::downgrade(&controller));
    #[cfg(test)]
    policy.attach(&attempt);
    let _guard = PairWindow(Arc::clone(&controller), Arc::clone(&attempt));
    controller.phase(PhonePhase::PeerVerification);
    policy.verify_peer(peer, profile.as_ref()).await?;
    eyre::ensure!(
        controller.pairing_is_current(&record.generation, &attempt),
        "pairing attempt unavailable"
    );
    let _work = engine.control_work()?;
    let mut binding = PairingBinding::dummy(wire::unix_ms()?)?;
    if let Some(profile) = &profile {
        binding.desktop_node = profile.desktop_node.as_str().to_owned();
        binding.phone_node = profile.phone_node.as_str().to_owned();
        binding.endpoint = format!("{}:{}", profile.desktop.get(), PORT);
    }
    binding.scope = super::SCOPE.into();
    let remaining = pair_until.saturating_duration_since(StdInstant::now());
    binding.pairing_expires_unix_ms = wire::unix_ms()? + u64::try_from(remaining.as_millis())?;
    binding.session_expires_unix_ms = binding.pairing_expires_unix_ms;
    let until = Instant::now() + remaining;
    let java = policy.java_executable(profile.as_ref())?;
    let artifacts = policy.pairing_runtime()?;
    controller.phase(PhonePhase::Pairing);
    let paired = policy
        .accept(
            socket,
            (&java, artifacts.as_path()),
            &binding,
            until,
            attempt,
            profile.as_ref(),
        )
        .await?;
    policy.verify_peer(peer, profile.as_ref()).await?;
    #[cfg(test)]
    if policy.test_only() {
        policy.event("before_enrollment");
        drop(paired);
        policy.event("socket_dropped_before_enrollment");
        return Ok(());
    }
    controller.phase(PhonePhase::IdentityEnrollment);
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
        ready: Arc<Mutex<Option<Sender<u16>>>>,
        events: Arc<super::faithful_integration::Events>,
    },
}
impl NetworkPolicy {
    fn pairing_runtime(&self) -> eyre::Result<PairingRuntime> {
        match self {
            Self::Pinned => PairingRuntime::installed(),
            #[cfg(test)]
            Self::Loopback { .. } => Ok(PairingRuntime::development_fixture()),
        }
    }

    async fn bind(&self) -> eyre::Result<PhoneListener> {
        #[cfg(test)]
        if let Self::Loopback { ready, events } = self {
            let listener = TcpListener::bind("127.0.0.1:0").await?;
            if let Some(sender) = ready
                .lock()
                .map_err(|_redacted_error| eyre::eyre!("test ready unavailable"))?
                .take()
            {
                let _ = sender.send(listener.local_addr()?.port());
            }
            events.record("TEAMY_TEST_ONLY_LOOPBACK_VERIFIER");
            return Ok(PhoneListener {
                socket: listener,
                profile: None,
            });
        }
        let profile = PhoneProfile::load(&AppHome::resolve()?)?;
        TailnetPeer::verify_desktop_for(&profile).await?;
        let listener = TcpListener::bind((profile.desktop.get(), PORT)).await?;
        TailnetPeer::require_current(&profile)?;
        Ok(PhoneListener {
            socket: listener,
            profile: Some(profile),
        })
    }
    fn permits_source(&self, peer: SocketAddr, profile: Option<&PhoneProfile>) -> bool {
        #[cfg(test)]
        if matches!(self, Self::Loopback { .. }) {
            return peer.ip().is_loopback();
        }
        match self {
            Self::Pinned => {
                profile.is_some_and(|profile| TailnetPeer::permits_source_for(peer, profile))
            }
            #[cfg(test)]
            Self::Loopback { .. } => peer.ip().is_loopback(),
        }
    }
    fn java_executable(&self, profile: Option<&PhoneProfile>) -> eyre::Result<PathBuf> {
        match self {
            Self::Pinned => {
                let profile = profile.ok_or_else(|| eyre::eyre!("phone profile unavailable"))?;
                TailnetPeer::require_current(profile)?;
                profile.java.require_available()?;
                Ok(profile.java.clone().into_path_buf())
            }
            #[cfg(test)]
            Self::Loopback { .. } => Ok(JavaExecutablePath::for_tests()?.into_path_buf()),
        }
    }
    async fn verify_peer(
        &self,
        peer: SocketAddr,
        profile: Option<&PhoneProfile>,
    ) -> eyre::Result<()> {
        #[cfg(test)]
        if matches!(self, Self::Loopback { .. }) {
            eyre::ensure!(peer.ip().is_loopback(), "test peer rejected");
            return Ok(());
        }
        TailnetPeer::verify_peer_for(
            peer,
            profile.ok_or_else(|| eyre::eyre!("phone profile unavailable"))?,
        )
        .await
    }
    async fn accept(
        &self,
        socket: TcpStream,
        runtime: (&Path, &Path),
        binding: &PairingBinding,
        until: Instant,
        attempt: Arc<super::pairing_confirmation::Attempt>,
        profile: Option<&PhoneProfile>,
    ) -> eyre::Result<pairing::paired_socket::PairedSocket> {
        let (java, artifacts) = runtime;
        #[cfg(test)]
        if self.test_only() {
            let peer = socket.peer_addr()?;
            return pairing::pairing_server::accept_test_loopback_socket(
                socket, peer, java, artifacts, binding, until, attempt,
            )
            .await;
        }
        pairing::pairing_server::accept_persistent_socket(
            socket,
            java,
            artifacts,
            binding,
            until,
            attempt,
            profile.ok_or_else(|| eyre::eyre!("phone profile unavailable"))?,
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

#[cfg(test)]
mod tests {
    use super::NetworkPolicy;
    use crate::broker::phone::config::phone_profile;
    use std::net::SocketAddr;

    #[test]
    fn pinned_source_requires_the_bound_profile() {
        let profile = phone_profile::synthetic();
        let expected = SocketAddr::from((profile.phone.get(), 50000));
        assert!(NetworkPolicy::Pinned.permits_source(expected, Some(&profile)));
        assert!(!NetworkPolicy::Pinned.permits_source(expected, None));
        assert!(
            !NetworkPolicy::Pinned
                .permits_source(SocketAddr::from(([127, 0, 0, 1], 50000)), Some(&profile))
        );
    }
}
