use super::*;
use rustls::pki_types::CertificateDer;
use rustls::pki_types::PrivatePkcs8KeyDer;
use rustls::pki_types::ServerName;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
pub(super) struct Fixture(
    std::path::PathBuf,
    Arc<crate::broker::shared_preview::SharedPreviewHub>,
);
impl Fixture {
    pub(super) fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("teamy-phone-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&dir).unwrap();
        Self(
            dir.join("identity.dpapi"),
            Arc::new(crate::broker::shared_preview::SharedPreviewHub::default()),
        )
    }
    pub(super) fn controller(&self) -> Arc<Controller> {
        let store = Store(self.0.clone());
        let record = store.load().unwrap();
        Arc::new(Controller {
            store,
            hub: PreviewHub::Isolated(Arc::clone(&self.1)),
            credential_hub: PreviewHub::Isolated(Arc::new(
                crate::broker::shared_preview::SharedPreviewHub::default(),
            )),
            record: Mutex::new(record),
            pair_until: Mutex::new(None),
            confirmation: Mutex::new(None),
            permission: Mutex::new(None),
            connected: AtomicBool::new(false),
            listener: AtomicBool::new(false),
            runtime: AtomicU8::new(0),
            actor_active: AtomicBool::new(false),
            diagnostic: Mutex::new(crate::protocol::phone_status::PhoneDiagnostic::default()),
        })
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        let _ = std::fs::remove_dir(self.0.parent().unwrap());
    }
}
fn phone() -> (Vec<u8>, Vec<u8>) {
    let key = rcgen::KeyPair::generate().unwrap();
    let mut params = rcgen::CertificateParams::new(vec!["phone.invalid".into()]).unwrap();
    params.not_before = time::OffsetDateTime::now_utc() - time::Duration::days(1);
    params.key_usages = vec![rcgen::KeyUsagePurpose::DigitalSignature];
    let cert = params.self_signed(&key).unwrap();
    (cert.der().to_vec(), key.serialize_der())
}
#[test]
fn protected_relationship_survives_restart_and_revoke() {
    let fixture = Fixture::new();
    let controller = fixture.controller();
    assert!(!controller.status().unwrap().paired);
    controller.arm().unwrap();
    let record = controller.record().unwrap().unwrap();
    let (certificate, _key) = phone();
    controller.enroll(&record.generation, &certificate).unwrap();
    let encrypted = std::fs::read(&fixture.0).unwrap();
    assert!(
        !encrypted
            .windows(record.private_key.len())
            .any(|w| w == record.private_key)
    );
    let restarted = fixture.controller();
    assert!(
        restarted
            .authorized(&record.generation, &certificate)
            .unwrap()
    );
    drop(restarted.arm().unwrap_err());
    restarted.revoke().unwrap();
    assert!(
        !restarted
            .authorized(&record.generation, &certificate)
            .unwrap()
    );
    assert!(!fixture.controller().status().unwrap().paired);
    std::fs::write(&fixture.0, b"tampered").unwrap();
    drop(Store(fixture.0.clone()).load().unwrap_err());
}
#[test]
fn strict_scope_version_sequence_session_and_preview_allowlist() {
    use message::Message;
    let mut message = Message {
        version: 1,
        scope: SCOPE.into(),
        session_id: "session".into(),
        sequence: 1,
        operation: "poll".into(),
        request_id: None,
        decision: None,
    };
    message.validate("session", 1).unwrap();
    drop(message.validate("another-session", 1).unwrap_err());
    drop(message.validate("session", 2).unwrap_err());
    message.version = 2;
    drop(message.validate("session", 1).unwrap_err());
    message.version = 1;
    message.scope = "vault".into();
    drop(message.validate("session", 1).unwrap_err());
    message.scope = SCOPE.into();
    message.operation = "acquire".into();
    drop(message.validate("session", 1).unwrap_err());
    message.operation = "decide".into();
    message.request_id = Some(uuid::Uuid::new_v4().to_string());
    message.decision = Some("approve".into());
    message.validate("session", 1).unwrap();
    message.decision = Some("send-credential".into());
    drop(message.validate("session", 1).unwrap_err());
}
fn client_config(record: &Record, certificate: Vec<u8>, key: Vec<u8>) -> rustls::ClientConfig {
    let mut roots = rustls::RootCertStore::empty();
    roots
        .add(CertificateDer::from(record.certificate.clone()))
        .unwrap();
    rustls::ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_protocol_versions(&[&rustls::version::TLS13])
        .unwrap()
        .with_root_certificates(roots)
        .with_client_auth_cert(
            vec![CertificateDer::from(certificate)],
            PrivatePkcs8KeyDer::from(key).into(),
        )
        .unwrap()
}
async fn connect(
    controller: Arc<Controller>,
    record: Record,
    certificate: Vec<u8>,
    key: Vec<u8>,
) -> eyre::Result<(
    tokio_rustls::client::TlsStream<tokio::net::TcpStream>,
    tokio::task::JoinHandle<eyre::Result<()>>,
)> {
    let config = client_config(&record, certificate, key);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let client = tokio::net::TcpStream::connect(listener.local_addr()?).await?;
    let (server, _) = listener.accept().await?;
    drop(listener);
    let task = tokio::spawn(transport::test_connection(server, controller, record));
    let tls = tokio_rustls::TlsConnector::from(Arc::new(config))
        .connect(
            ServerName::IpAddress(teamy_broker_handoff::phone_config::DESKTOP.into()),
            client,
        )
        .await?;
    Ok((tls, task))
}
async fn send(
    tls: &mut tokio_rustls::client::TlsStream<tokio::net::TcpStream>,
    session: &str,
    sequence: u64,
    id: Option<String>,
    decision: Option<&str>,
) -> eyre::Result<message::Reply> {
    let message = message::Message {
        version: 1,
        scope: SCOPE.into(),
        session_id: session.into(),
        sequence,
        operation: if id.is_some() { "decide" } else { "poll" }.into(),
        request_id: id,
        decision: decision.map(str::to_string),
    };
    crate::protocol::wire::write(tls, &message).await?;
    tls.flush().await?;
    crate::protocol::wire::read(tls).await
}
#[tokio::test]
async fn tls_reconnect_reconcile_stale_replay_and_live_revocation() -> eyre::Result<()> {
    let fixture = Fixture::new();
    let controller = fixture.controller();
    controller.arm()?;
    let initial = controller.record()?.unwrap();
    let (cert, key) = phone();
    controller.enroll(&initial.generation, &cert)?;
    let record = controller.record()?.unwrap();
    let (mut tls, task) = connect(
        Arc::clone(&controller),
        record.clone(),
        cert.clone(),
        key.clone(),
    )
    .await?;
    let hello: message::Reply = crate::protocol::wire::read(&mut tls).await?;
    let hub = &fixture.1;
    approve_then_reconcile(&mut tls, &hello.session_id, hub).await?;
    drop(tls);
    drop(task.await?.unwrap_err());
    let restarted = fixture.controller();
    let (mut tls, task) = connect(
        Arc::clone(&restarted),
        restarted.record()?.unwrap(),
        cert.clone(),
        key.clone(),
    )
    .await?;
    let next: message::Reply = crate::protocol::wire::read(&mut tls).await?;
    assert_ne!(hello.session_id, next.session_id);
    assert!(next.pending.is_none());
    let request = hub.begin(Arc::new(AtomicBool::new(false)))?;
    let pending = send(&mut tls, &next.session_id, 1, None, None)
        .await?
        .pending
        .unwrap();
    hub.decide(
        &pending.request_id,
        crate::protocol::ApprovalPreviewDisposition::DeniedTest,
    )?;
    assert!(
        send(&mut tls, &next.session_id, 2, None, None)
            .await?
            .pending
            .is_none()
    );
    assert_eq!(
        request.outcome(),
        Some(crate::protocol::ApprovalPreviewDisposition::DeniedTest)
    );
    drop(
        send(&mut tls, &hello.session_id, 3, None, None)
            .await
            .unwrap_err(),
    );
    drop(task.await?.unwrap_err());
    let (mut tls, task) = connect(
        Arc::clone(&restarted),
        restarted.record()?.unwrap(),
        cert.clone(),
        key,
    )
    .await?;
    let final_hello: message::Reply = crate::protocol::wire::read(&mut tls).await?;
    restarted.revoke()?;
    drop(
        send(&mut tls, &final_hello.session_id, 1, None, None)
            .await
            .unwrap_err(),
    );
    drop(task.await?.unwrap_err());
    restarted.revoke()?;
    assert!(!restarted.authorized(&initial.generation, &cert)?);
    Ok(())
}

#[tokio::test]
async fn unpaired_revoked_and_different_certificate_rejected() -> eyre::Result<()> {
    let fixture = Fixture::new();
    let controller = fixture.controller();
    controller.arm()?;
    let initial = controller.record()?.unwrap();
    drop(
        super::identity::acceptor(&initial)
            .err()
            .expect("unpaired rejection"),
    );
    let (cert, key) = phone();
    controller.enroll(&initial.generation, &cert)?;
    let (other_cert, other_key) = phone();
    let (mut tls, task) = connect(
        Arc::clone(&controller),
        controller.record()?.unwrap(),
        other_cert,
        other_key,
    )
    .await?;
    let response: eyre::Result<message::Reply> = crate::protocol::wire::read(&mut tls).await;
    drop(response.unwrap_err());
    drop(task.await?.unwrap_err());
    controller.revoke()?;
    drop(
        super::identity::acceptor(&controller.record()?.unwrap())
            .err()
            .expect("revoked rejection"),
    );
    assert!(!controller.authorized(&initial.generation, &cert)?);
    drop(key);
    Ok(())
}

async fn approve_then_reconcile(
    tls: &mut tokio_rustls::client::TlsStream<tokio::net::TcpStream>,
    session: &str,
    hub: &crate::broker::shared_preview::SharedPreviewHub,
) -> eyre::Result<()> {
    let request = hub.begin(Arc::new(AtomicBool::new(false)))?;
    let pending = send(tls, session, 1, None, None).await?.pending.unwrap();
    assert_eq!(
        send(
            tls,
            session,
            2,
            Some(pending.request_id.clone()),
            Some("approve")
        )
        .await?
        .accepted,
        Some(true)
    );
    assert_eq!(
        request.outcome(),
        Some(crate::protocol::ApprovalPreviewDisposition::ApprovedTest)
    );
    assert_eq!(
        send(
            tls,
            session,
            3,
            Some(pending.request_id.clone()),
            Some("deny")
        )
        .await?
        .accepted,
        Some(false)
    );
    assert!(send(tls, session, 4, None, None).await?.pending.is_none());
    Ok(())
}

#[tokio::test]
async fn offline_pending_survives_connection_loss_but_cancellation_reconciles() -> eyre::Result<()>
{
    let fixture = Fixture::new();
    let controller = fixture.controller();
    controller.arm()?;
    let initial = controller.record()?.unwrap();
    let (cert, key) = phone();
    controller.enroll(&initial.generation, &cert)?;
    let cancellation = Arc::new(AtomicBool::new(false));
    let request = fixture.1.begin(Arc::clone(&cancellation))?;
    let (mut tls, task) = connect(
        Arc::clone(&controller),
        controller.record()?.unwrap(),
        cert.clone(),
        key.clone(),
    )
    .await?;
    let hello: message::Reply = crate::protocol::wire::read(&mut tls).await?;
    let pending = hello.pending.unwrap();
    let remaining = pending.expires_unix_ms - crate::protocol::wire::unix_ms()?;
    assert!(remaining > 0 && remaining <= 60000);
    drop(tls);
    drop(task.await?.unwrap_err());
    assert!(request.outcome().is_none());
    assert_eq!(fixture.1.snapshot().unwrap().request_id, pending.request_id);
    let restarted = fixture.controller();
    let (mut tls, task) = connect(
        Arc::clone(&restarted),
        restarted.record()?.unwrap(),
        cert,
        key,
    )
    .await?;
    let next: message::Reply = crate::protocol::wire::read(&mut tls).await?;
    assert_eq!(next.pending.unwrap().request_id, pending.request_id);
    cancellation.store(true, Ordering::Release);
    assert!(
        send(&mut tls, &next.session_id, 1, None, None)
            .await?
            .pending
            .is_none()
    );
    assert_eq!(
        send(
            &mut tls,
            &next.session_id,
            2,
            Some(pending.request_id),
            Some("approve")
        )
        .await?
        .accepted,
        Some(false)
    );
    assert_eq!(
        request.outcome(),
        Some(crate::protocol::ApprovalPreviewDisposition::Cancelled)
    );
    drop(tls);
    drop(task.await?.unwrap_err());
    Ok(())
}

#[tokio::test]
async fn listener_guard_clears_status_on_error_and_abort() -> eyre::Result<()> {
    let fixture = Fixture::new();
    let controller = fixture.controller();
    controller.listener.store(true, Ordering::Release);
    controller.connected.store(true, Ordering::Release);
    let result = super::resident::guarded(Arc::clone(&controller), async {
        eyre::bail!("injected accept failure")
    })
    .await;
    drop(result.unwrap_err());
    assert!(!controller.status()?.listener);
    assert_eq!(
        controller.status()?.connection,
        crate::protocol::phone_status::PhoneConnectionState::Disconnected
    );
    let (ready, observed) = tokio::sync::oneshot::channel();
    let owned = Arc::clone(&controller);
    let task = tokio::spawn(super::resident::guarded(
        Arc::clone(&controller),
        async move {
            owned.listener.store(true, Ordering::Release);
            owned.connected.store(true, Ordering::Release);
            let _ = ready.send(());
            std::future::pending().await
        },
    ));
    observed.await?;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert!(!controller.status()?.listener);
    assert_eq!(
        controller.status()?.connection,
        crate::protocol::phone_status::PhoneConnectionState::Disconnected
    );
    Ok(())
}
#[test]
fn runtime_guard_retains_failure_and_releases_actor_ownership() {
    let fixture = Fixture::new();
    let controller = fixture.controller();
    controller.actor_active.store(true, Ordering::Release);
    let guard = RuntimeGuard(Arc::clone(&controller));
    controller.runtime.store(3, Ordering::Release);
    controller.listener.store(true, Ordering::Release);
    controller.connected.store(true, Ordering::Release);
    drop(guard);
    assert_eq!(
        controller.status().unwrap().runtime,
        crate::protocol::phone_status::PhoneRuntimeState::Failed
    );
    assert!(!controller.status().unwrap().listener);
    assert!(!controller.actor_active.load(Ordering::Acquire));
    drop(controller.arm().unwrap_err());
}

#[test]
fn enrollment_window_is_five_minutes_without_renewal_or_double_arm() {
    let fixture = Fixture::new();
    let controller = fixture.controller();
    controller.arm().unwrap();
    let deadline = controller.pair_until.lock().unwrap().unwrap();
    let remaining = deadline.saturating_duration_since(Instant::now());
    assert!(remaining > std::time::Duration::from_secs(299) && remaining <= ENROLLMENT_WINDOW);
    drop(controller.arm().unwrap_err());
    assert_eq!(controller.pair_until.lock().unwrap().unwrap(), deadline);
    *controller.pair_until.lock().unwrap() = Some(Instant::now());
    assert!(!controller.pairing_live().unwrap());
    let record = controller.record().unwrap().unwrap();
    let (cert, _) = phone();
    drop(controller.enroll(&record.generation, &cert).unwrap_err());
}

#[test]
fn diagnostic_failure_is_static_bounded_and_survives_next_attempt() {
    use crate::protocol::phone_status::PhoneFailureCategory;
    use crate::protocol::phone_status::PhonePhase;
    let fixture = Fixture::new();
    let controller = fixture.controller();
    let attempt = diagnostic::Attempt::begin(Arc::clone(&controller));
    controller.phase(PhonePhase::TlsHandshake);
    controller.failure(PhoneFailureCategory::CertificateUnknownIssuer);
    attempt.finish(false);
    let first = controller
        .status()
        .unwrap()
        .diagnostic
        .last_failure
        .unwrap();
    assert_eq!(
        first.category,
        PhoneFailureCategory::CertificateUnknownIssuer
    );
    let next = diagnostic::Attempt::begin(Arc::clone(&controller));
    assert_eq!(
        controller
            .status()
            .unwrap()
            .diagnostic
            .last_failure
            .unwrap()
            .attempt,
        first.attempt
    );
    controller.phase(PhonePhase::TlsHandshake);
    next.finish(false);
    let status = controller.status().unwrap().diagnostic;
    assert_eq!(
        status.last_failure.unwrap().category,
        PhoneFailureCategory::Unavailable
    );
    assert!(status.last_failure.unwrap().attempt > first.attempt);
    assert!(facet_json::to_string(&status).unwrap().len() < 512);
    let error = std::io::Error::other(rustls::Error::InvalidCertificate(
        rustls::CertificateError::InvalidPurpose,
    ));
    assert_eq!(
        diagnostic::tls_category(&error),
        PhoneFailureCategory::CertificateInvalidPurpose
    );
}

#[test]
fn phone_hello_has_explicit_null_fields_required_by_android() {
    let reply = message::Reply {
        version: 1,
        scope: SCOPE.into(),
        session_id: uuid::Uuid::nil().to_string(),
        sequence: 0,
        pending: None,
        accepted: None,
    };
    let json = facet_json::to_string(&reply).unwrap();
    assert!(json.contains("\"pending\":null"));
    assert!(json.contains("\"accepted\":null"));
}

#[tokio::test]
async fn request_bound_supply_binary_tls_frame_and_one_use_permission() -> eyre::Result<()> {
    let fixture = Fixture::new();
    let controller = fixture.controller();
    controller.arm()?;
    let initial = controller.record()?.unwrap();
    let (cert, key) = phone();
    controller.enroll(&initial.generation, &cert)?;
    *controller.pair_until.lock().unwrap() = None;
    let request = crate::protocol::AcquireRequest {
        account: "Synthetic".into(),
        secret_ref: "op://Synthetic/Dummy/token".into(),
        requesting_app: "synthetic-discord".into(),
        purpose: "dummy read-only messages".into(),
        destination: "https://discord.invalid/dummy".into(),
        backend: crate::protocol::Backend::Memory,
    };
    assert!(controller.claim_permission(&request)?.is_none());
    controller.authorize_once(request.clone(), true)?;
    let granted = controller.claim_permission(&request)?.unwrap();
    assert!(granted.supply);
    assert!(controller.claim_permission(&request)?.is_none());
    let shared = granted.begin(request, 1, Arc::new(AtomicBool::new(false)))?;
    let record = controller.record()?.unwrap();
    let (mut tls, task) = connect(Arc::clone(&controller), record, cert, key).await?;
    let hello: message::Reply = crate::protocol::wire::read(&mut tls).await?;
    let pending = hello.pending.unwrap();
    assert!(pending.credential.is_some());
    let message = message::Message {
        version: 1,
        scope: SCOPE.into(),
        session_id: hello.session_id.clone(),
        sequence: 1,
        operation: "supply".into(),
        request_id: Some(pending.request_id.clone()),
        decision: None,
    };
    crate::protocol::wire::write(&mut tls, &message).await?;
    crate::protocol::wire::write_secret(&mut tls, b"synthetic-tls-value").await?;
    let reply: message::Reply = crate::protocol::wire::read(&mut tls).await?;
    assert_eq!(reply.accepted, Some(true));
    assert!(reply.pending.is_none());
    assert_eq!(&*shared.take_supply().unwrap(), b"synthetic-tls-value");
    let stale = message::Message {
        sequence: 2,
        ..message
    };
    crate::protocol::wire::write(&mut tls, &stale).await?;
    crate::protocol::wire::write_secret(&mut tls, b"late-synthetic").await?;
    let reply: message::Reply = crate::protocol::wire::read(&mut tls).await?;
    assert_eq!(reply.accepted, Some(false));
    assert!(shared.take_supply().is_none());
    drop(tls);
    drop(task.await?.unwrap_err());
    Ok(())
}

use crate::broker::engine::tests::shared_approval as granted_approval;
use crate::broker::engine::tests::support as credential_support;
fn credential_fixture() -> eyre::Result<(Fixture, Arc<Controller>, crate::protocol::AcquireRequest)>
{
    let fixture = Fixture::new();
    let controller = fixture.controller();
    controller.arm()?;
    let initial = controller.record()?.unwrap();
    let (cert, _key) = phone();
    controller.enroll(&initial.generation, &cert)?;
    *controller.pair_until.lock().unwrap() = None;
    let request = crate::protocol::AcquireRequest {
        account: "Synthetic".into(),
        secret_ref: "op://Synthetic/Dummy/token".into(),
        requesting_app: "synthetic-discord".into(),
        purpose: "synthetic race".into(),
        destination: "https://discord.invalid/dummy".into(),
        backend: crate::protocol::Backend::Memory,
    };
    Ok((fixture, controller, request))
}
#[test]
fn revoke_after_consumption_before_request_creation_rejects_grant() -> eyre::Result<()> {
    let (_fixture, controller, request) = credential_fixture()?;
    controller.authorize_once(request.clone(), true)?;
    let grant = controller.claim_permission(&request)?.unwrap();
    controller.revoke()?;
    drop(
        grant
            .begin(request, 1, Arc::new(AtomicBool::new(false)))
            .unwrap_err(),
    );
    assert!(controller.credential_hub().snapshot().is_none());
    Ok(())
}
async fn reject_late_publication(revoke: bool) -> eyre::Result<()> {
    let (_fixture, controller, request) = credential_fixture()?;
    controller.authorize_once(request.clone(), true)?;
    let grant = controller.claim_permission(&request)?.unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let shared = grant.begin(request.clone(), 1, Arc::clone(&cancel))?;
    assert!(shared.supply(zeroize::Zeroizing::new(b"synthetic-winner".to_vec())));
    let storage = Arc::new(credential_support::fake_storage::FakeStorage::default());
    let engine = Arc::new(crate::broker::engine::Engine::new(
        Arc::new(credential_support::fake_adapter::FakeAdapter::default()),
        Arc::new(granted_approval::SharedApproval(Arc::clone(&shared))),
        Arc::<credential_support::fake_storage::FakeStorage>::clone(&storage),
    ));
    let gate = Arc::new(crate::broker::publication_gate::PublicationGate::default());
    *engine.publication_gate.lock().unwrap() = Some(Arc::clone(&gate));
    let running = Arc::clone(&engine);
    let second_request = request.clone();
    let first_cancel = Arc::clone(&cancel);
    let task = tokio::spawn(async move { running.acquire(request, 1, cancel).await });
    tokio::time::timeout(std::time::Duration::from_secs(2), gate.ready.notified()).await?;
    if revoke {
        controller.authorize_once(second_request.clone(), true)?;
        let replacement = controller.claim_permission(&second_request)?.unwrap();
        let second = replacement.begin(second_request, 2, Arc::new(AtomicBool::new(false)))?;
        controller.revoke()?;
        assert!(!first_cancel.load(Ordering::SeqCst));
        assert_eq!(
            second.outcome(),
            Some(crate::protocol::ApprovalPreviewDisposition::Cancelled)
        );
    } else {
        shared.expire_original_deadline();
    }
    gate.release.notify_one();
    drop(task.await?.unwrap_err());
    assert!(storage.published.lock().unwrap().is_empty());
    assert_eq!(engine.status().await?.active_leases, 0);
    Ok(())
}
#[tokio::test]
async fn revoke_after_value_win_before_cache_insertion_cannot_publish() -> eyre::Result<()> {
    reject_late_publication(true).await
}
#[tokio::test]
async fn original_deadline_expiry_after_value_win_before_cache_insertion_cannot_publish()
-> eyre::Result<()> {
    reject_late_publication(false).await
}

struct FixtureApproval;
impl crate::broker::approval::Approval for FixtureApproval {
    fn approve<'a>(
        &'a self,
        _request: &'a crate::protocol::AcquireRequest,
        _pid: u32,
        _cancel: Arc<AtomicBool>,
        _work: crate::broker::work_guard::WorkGuard,
    ) -> std::pin::Pin<Box<dyn Future<Output = eyre::Result<bool>> + Send + 'a>> {
        Box::pin(async { eyre::bail!("normal approval forbidden") })
    }
    fn fixture_decision<'a>(
        &'a self,
        request: &'a crate::protocol::AcquireRequest,
        pid: u32,
        cancel: Arc<AtomicBool>,
        _work: crate::broker::work_guard::WorkGuard,
        grant: Arc<crate::broker::credential_grant::CredentialGrant>,
    ) -> std::pin::Pin<
        Box<
            dyn Future<Output = eyre::Result<crate::broker::approval_decision::ApprovalDecision>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let shared = grant.begin(request.clone(), pid, cancel)?;
            assert!(shared.synthetic_fixture());
            assert!(!shared.supply(zeroize::Zeroizing::new(
                b"real-looking-arbitrary-input".to_vec()
            )));
            assert!(shared.request_provider());
            Ok(crate::broker::approval_decision::ApprovalDecision::Shared(
                shared,
            ))
        })
    }
}
#[tokio::test]
async fn typed_fixture_ipc_rejects_missing_expired_consumed_grants_and_ignores_live_cache()
-> eyre::Result<()> {
    let (_fixture, controller, _) = credential_fixture()?;
    let request = crate::broker::synthetic_credential_fixture::request();
    let adapter = Arc::new(credential_support::fake_adapter::FakeAdapter::default());
    let storage = Arc::new(credential_support::fake_storage::FakeStorage::default());
    let engine = Arc::new(crate::broker::engine::Engine::new(
        Arc::<credential_support::fake_adapter::FakeAdapter>::clone(&adapter),
        Arc::new(FixtureApproval),
        Arc::<credential_support::fake_storage::FakeStorage>::clone(&storage),
    ));
    *engine.fixture_controller.lock().unwrap() = Some(Arc::clone(&controller));
    let cached = crate::broker::lease::Lease::new(
        request.clone(),
        zeroize::Zeroizing::new(b"ordinary-live-cache-match".to_vec()),
        crate::protocol::wire::unix_ms()?,
        std::time::Instant::now(),
    )?;
    let cache_id = cached.id.clone();
    engine.leases.lock().await.insert(cache_id.clone(), cached);
    let endpoint = format!(
        r"\\.\pipe\teamy-broker-test-fixture-{}",
        uuid::Uuid::new_v4()
    );
    let client = crate::sdk::Client::new(None)?.with_endpoint(endpoint.clone());
    let serving = Arc::clone(&engine);
    let stop = teamy_cancellation::CancellationToken::new();
    let stopped = stop.clone();
    let task = tokio::spawn(async move {
        crate::broker::server::serve_at(serving, stopped, &endpoint, false).await
    });
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while !client.status().await?.running {
            tokio::task::yield_now().await;
        }
        Ok::<_, eyre::Report>(())
    })
    .await??;
    drop(client.acquire_fixture(request.clone()).await.unwrap_err());
    controller.authorize_fixture(request.clone())?;
    controller
        .permission
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .expire_for_test();
    drop(client.acquire_fixture(request.clone()).await.unwrap_err());
    controller.authorize_fixture(request.clone())?;
    drop(controller.claim_fixture_permission(&request)?);
    drop(client.acquire_fixture(request.clone()).await.unwrap_err());
    assert_eq!(adapter.calls.load(Ordering::SeqCst), 0);
    assert!(storage.published.lock().unwrap().is_empty());
    controller.authorize_fixture(request.clone())?;
    let value = client.acquire_fixture(request.clone()).await?;
    assert!(value.expose(|bytes| bytes == crate::broker::synthetic_credential_fixture::PROVIDER)?);
    assert_ne!(value.lease_id, cache_id);
    assert_eq!(adapter.calls.load(Ordering::SeqCst), 0);
    assert_eq!(storage.published.lock().unwrap().len(), 1);
    drop(client.acquire_fixture(request.clone()).await.unwrap_err());
    drop(client.acquire(request).await.unwrap_err());
    assert_eq!(adapter.calls.load(Ordering::SeqCst), 0);
    client.purge(Some(value.lease_id)).await?;
    client.stop_force().await?;
    task.await??;
    Ok(())
}
#[test]
fn synthetic_vault_name_and_changed_fixture_metadata_cannot_select_fixture() -> eyre::Result<()> {
    let (_fixture, controller, live) = credential_fixture()?;
    assert!(live.secret_ref.starts_with("op://Synthetic/"));
    assert!(controller.authorize_fixture(live.clone()).is_err());
    controller.authorize_once(live.clone(), true)?;
    assert!(
        !controller
            .claim_permission(&live)?
            .unwrap()
            .synthetic_fixture
    );
    for field in 0..6 {
        let mut changed = crate::broker::synthetic_credential_fixture::request();
        match field {
            0 => changed.account.push('x'),
            1 => changed.secret_ref.push('x'),
            2 => changed.requesting_app.push('x'),
            3 => changed.purpose.push('x'),
            4 => changed.destination.push('x'),
            _ => {
                changed.backend = crate::protocol::Backend::WindowsCredentialManager(
                    "teamy-broker/fixture-spoof".into(),
                );
            }
        }
        assert!(controller.authorize_fixture(changed).is_err());
    }
    Ok(())
}
#[tokio::test]
async fn message_read_distinguishes_timeout_closed_and_rejected_frames() -> eyre::Result<()> {
    use crate::protocol::phone_status::PhoneFailureCategory as F;
    use crate::protocol::phone_status::PhonePhase;
    let (_fixture, controller, _) = credential_fixture()?;
    for expected in [F::Deadline, F::ReadClosed, F::MessageRejected] {
        let (mut reader, mut writer) = tokio::io::duplex(64);
        controller.phase(PhonePhase::MessageRead);
        if expected == F::ReadClosed {
            drop(writer);
        } else if expected == F::MessageRejected {
            writer.write_u32(16385).await?;
            drop(writer);
        }
        drop(
            super::message_read::read(
                &mut reader,
                &controller,
                std::time::Duration::from_millis(20),
            )
            .await
            .unwrap_err(),
        );
        assert_eq!(
            controller
                .status()?
                .diagnostic
                .last_failure
                .unwrap()
                .category,
            expected
        );
    }
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn idle_scheduling_slack_does_not_allow_slow_partial_frames() -> eyre::Result<()> {
    use crate::protocol::phone_status::PhoneFailureCategory as F;
    use crate::protocol::phone_status::PhonePhase;
    let (_fixture, controller, _) = credential_fixture()?;
    let (mut reader, mut writer) = tokio::io::duplex(1024);
    let owned = Arc::clone(&controller);
    let read = tokio::spawn(async move {
        owned.phase(PhonePhase::MessageRead);
        super::message_read::read(&mut reader, &owned, super::timing::IDLE).await
    });
    tokio::time::sleep(std::time::Duration::from_secs(7)).await;
    crate::protocol::wire::write(
        &mut writer,
        &message::Message {
            version: 1,
            scope: SCOPE.into(),
            session_id: "idle-test".into(),
            sequence: 1,
            operation: "poll".into(),
            request_id: None,
            decision: None,
        },
    )
    .await?;
    assert_eq!(read.await??.sequence, 1);
    let (mut reader, mut writer) = tokio::io::duplex(64);
    let owned = Arc::clone(&controller);
    let read = tokio::spawn(async move {
        super::message_read::read(&mut reader, &owned, super::timing::IDLE).await
    });
    writer.write_u8(0).await?;
    tokio::time::sleep(std::time::Duration::from_secs(4)).await;
    writer.write_u8(0).await?;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    drop(read.await?.unwrap_err());
    assert_eq!(
        controller
            .status()?
            .diagnostic
            .last_failure
            .unwrap()
            .category,
        F::Deadline
    );
    Ok(())
}
async fn slow_tls(
    controller: Arc<Controller>,
    record: Record,
    cert: Vec<u8>,
    key: Vec<u8>,
) -> eyre::Result<(
    tokio_rustls::client::TlsStream<tokio::io::DuplexStream>,
    tokio::task::JoinHandle<eyre::Result<()>>,
)> {
    let config = client_config(&record, cert, key);
    let (client, server) = tokio::io::duplex(32768);
    let task = tokio::spawn(transport::test_slow_connection(server, controller, record));
    let connector = tokio_rustls::TlsConnector::from(Arc::new(config));
    let tls = tokio::time::timeout(
        super::timing::CLIENT_CONNECT,
        connector.connect(
            ServerName::IpAddress(teamy_broker_handoff::phone_config::DESKTOP.into()),
            client,
        ),
    )
    .await??;
    Ok((tls, task))
}
#[tokio::test(start_paused = true)]
async fn slow_authorized_tls_supply_and_graceful_reconnect_keep_deadlines_and_sessions()
-> eyre::Result<()> {
    let (_fixture, controller, _) = credential_fixture()?;
    let (cert, key) = phone();
    // Replace only the isolated fixture's test certificate, never the resident relationship.
    controller.revoke()?;
    controller.arm()?;
    let initial = controller.record()?.unwrap();
    controller.enroll(&initial.generation, &cert)?;
    *controller.pair_until.lock().unwrap() = None;
    let request = crate::broker::synthetic_credential_fixture::request();
    controller.authorize_fixture(request.clone())?;
    let cancel = Arc::new(AtomicBool::new(false));
    let shared =
        controller
            .claim_fixture_permission(&request)?
            .begin(request, 1, Arc::clone(&cancel))?;
    let (mut tls, task) = slow_tls(
        Arc::clone(&controller),
        controller.record()?.unwrap(),
        cert.clone(),
        key.clone(),
    )
    .await?;
    let hello: message::Reply = tokio::time::timeout(
        super::timing::CLIENT_CONNECT,
        crate::protocol::wire::read(&mut tls),
    )
    .await??;
    let id = hello.pending.unwrap().request_id;
    tokio::time::sleep(std::time::Duration::from_secs(7)).await;
    let supply = message::Message {
        version: 1,
        scope: SCOPE.into(),
        session_id: hello.session_id.clone(),
        sequence: 1,
        operation: "supply".into(),
        request_id: Some(id.clone()),
        decision: None,
    };
    let reply: message::Reply = tokio::time::timeout(super::timing::CLIENT_EXCHANGE, async {
        crate::protocol::wire::write(&mut tls, &supply).await?;
        crate::protocol::wire::write_secret(
            &mut tls,
            crate::broker::synthetic_credential_fixture::INPUT,
        )
        .await?;
        crate::protocol::wire::read(&mut tls).await
    })
    .await??;
    assert_eq!(reply.accepted, Some(true));
    assert!(reply.pending.is_none());
    assert_eq!(
        shared.take_supply().unwrap().as_slice(),
        crate::broker::synthetic_credential_fixture::INPUT
    );
    tls.shutdown().await?;
    drop(tls);
    drop(task.await?.unwrap_err());
    assert_eq!(
        controller
            .status()?
            .diagnostic
            .last_failure
            .unwrap()
            .category,
        crate::protocol::phone_status::PhoneFailureCategory::ReadClosed
    );
    let (mut tls, task) = slow_tls(
        Arc::clone(&controller),
        controller.record()?.unwrap(),
        cert,
        key,
    )
    .await?;
    let fresh: message::Reply = tokio::time::timeout(
        super::timing::CLIENT_CONNECT,
        crate::protocol::wire::read(&mut tls),
    )
    .await??;
    assert_ne!(fresh.session_id, hello.session_id);
    assert!(fresh.pending.is_none());
    crate::protocol::wire::write(&mut tls, &supply).await?;
    let rejected: eyre::Result<message::Reply> = crate::protocol::wire::read(&mut tls).await;
    drop(rejected.unwrap_err());
    drop(task.await?.unwrap_err());
    assert!(!cancel.load(Ordering::SeqCst));
    Ok(())
}

#[tokio::test(start_paused = true)]
async fn slow_exchange_never_renews_request_deadline_or_accepts_cancelled_supply()
-> eyre::Result<()> {
    for expire in [false, true] {
        let (_fixture, controller, _) = credential_fixture()?;
        let (cert, key) = phone();
        controller.revoke()?;
        controller.arm()?;
        let initial = controller.record()?.unwrap();
        controller.enroll(&initial.generation, &cert)?;
        *controller.pair_until.lock().unwrap() = None;
        let request = crate::broker::synthetic_credential_fixture::request();
        controller.authorize_fixture(request.clone())?;
        let cancel = Arc::new(AtomicBool::new(false));
        let shared = controller.claim_fixture_permission(&request)?.begin(
            request,
            1,
            Arc::clone(&cancel),
        )?;
        let (mut tls, task) = slow_tls(
            Arc::clone(&controller),
            controller.record()?.unwrap(),
            cert,
            key,
        )
        .await?;
        let hello: message::Reply = crate::protocol::wire::read(&mut tls).await?;
        let supply = message::Message {
            version: 1,
            scope: SCOPE.into(),
            session_id: hello.session_id,
            sequence: 1,
            operation: "supply".into(),
            request_id: Some(hello.pending.unwrap().request_id),
            decision: None,
        };
        let (exchange, ()) = tokio::join!(
            async {
                tokio::time::timeout(super::timing::CLIENT_EXCHANGE, async {
                    crate::protocol::wire::write(&mut tls, &supply).await?;
                    crate::protocol::wire::write_secret(
                        &mut tls,
                        crate::broker::synthetic_credential_fixture::INPUT,
                    )
                    .await?;
                    crate::protocol::wire::read::<message::Reply, _>(&mut tls).await
                })
                .await
            },
            async {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                if expire {
                    shared.expire_original_deadline();
                } else {
                    cancel.store(true, Ordering::SeqCst);
                }
            }
        );
        let reply = exchange??;
        assert_eq!(reply.accepted, Some(false));
        assert!(reply.pending.is_none());
        assert!(shared.take_supply().is_none());
        assert_eq!(
            shared.outcome(),
            Some(if expire {
                crate::protocol::ApprovalPreviewDisposition::TimedOut
            } else {
                crate::protocol::ApprovalPreviewDisposition::Cancelled
            })
        );
        tls.shutdown().await?;
        drop(tls);
        drop(task.await?.unwrap_err());
    }
    Ok(())
}

#[test]
fn stale_pairing_cleanup_cannot_cancel_new_attempt() {
    let f = Fixture::new();
    let c = f.controller();
    c.arm().unwrap();
    let old = Arc::clone(c.confirmation.lock().unwrap().as_ref().unwrap());
    c.revoke().unwrap();
    c.arm().unwrap();
    let fresh = Arc::clone(c.confirmation.lock().unwrap().as_ref().unwrap());
    c.finish_pairing_attempt(&old);
    assert!(c.pairing_live().unwrap());
    assert_eq!(
        c.status().unwrap().pairing_attempt_id,
        Some(fresh.id.clone())
    );
    assert!(!fresh.finished());
    c.revoke().unwrap();
    assert!(fresh.finished());
}

#[test]
fn acquisition_boundary_pins_generation_window_attempt() {
    let f = Fixture::new();
    let c = f.controller();
    c.arm().unwrap();
    let (identity, until, old) = c.capture_pairing().unwrap();
    assert!(until > Instant::now());
    c.revoke().unwrap();
    c.arm().unwrap();
    assert!(!c.pairing_is_current(&identity.generation, &old));
    let (new_identity, _, fresh) = c.capture_pairing().unwrap();
    assert_ne!(identity.generation, new_identity.generation);
    assert_ne!(old.id, fresh.id);
    c.finish_pairing_attempt(&old);
    assert!(c.pairing_is_current(&new_identity.generation, &fresh));
    c.revoke().unwrap();
}

fn isolated_pair_pipe() -> (String, tokio::net::windows::named_pipe::NamedPipeServer) {
    let endpoint = format!(r"\\.\pipe\teamy-pairing-test-{}", uuid::Uuid::new_v4());
    let pipe = crate::platform::pipe_security::create(&endpoint, true).unwrap();
    (endpoint, pipe)
}
async fn pair_pipe_hello(pipe: &mut tokio::net::windows::named_pipe::NamedPipeServer) {
    pipe.connect().await.unwrap();
    crate::protocol::wire::write(
        pipe,
        &crate::protocol::Hello {
            version: crate::protocol::VERSION,
            instance_id: uuid::Uuid::new_v4().to_string(),
            capabilities: crate::protocol::CAPABILITIES
                .iter()
                .map(|s| (*s).into())
                .chain(std::iter::once("persistent-no-vault-phone-v1".into()))
                .collect(),
        },
    )
    .await
    .unwrap();
    let _: crate::protocol::Hello = crate::protocol::wire::read(pipe).await.unwrap();
}
#[tokio::test]
async fn secured_pipe_dispatch_inspect_confirm_and_replay() {
    let f = Fixture::new();
    let c = f.controller();
    c.arm().unwrap();
    let (_, _, attempt) = c.capture_pairing().unwrap();
    attempt
        .publish(zeroize::Zeroizing::new("ABCD 1234 DCBA 5678".into()))
        .unwrap();
    for step in 0..3 {
        let (endpoint, mut pipe) = isolated_pair_pipe();
        let server = Arc::clone(&c);
        let task = tokio::spawn(async move {
            pair_pipe_hello(&mut pipe).await;
            let request: crate::protocol::Request =
                crate::protocol::wire::read(&mut pipe).await.unwrap();
            request.validate().unwrap();
            let peer = crate::protocol::Hello {
                version: crate::protocol::VERSION,
                instance_id: "test".into(),
                capabilities: vec!["persistent-no-vault-phone-v1".into()],
            };
            super::super::phone_control_session::execute_with(&mut pipe, &peer, &request, &server)
                .await
        });
        let client = crate::sdk::Client::new(None)
            .unwrap()
            .with_endpoint(endpoint)
            .with_deadlines(
                std::time::Duration::from_secs(1),
                std::time::Duration::from_secs(1),
            );
        if step == 0 {
            let result = client.pairing_inspect(&attempt.id).await.unwrap();
            assert_eq!(result.comparison, "ABCD 1234 DCBA 5678");
        } else {
            let code = crate::protocol::pairing_comparison::PairingCode {
                comparison: "ABCD 1234 DCBA 5678".into(),
            };
            if step == 1 {
                assert!(
                    client
                        .pairing_confirm(&attempt.id, &code)
                        .await
                        .unwrap()
                        .accepted
                );
            } else {
                let _ = client
                    .pairing_confirm(&attempt.id, &code)
                    .await
                    .expect_err("replay must close/reject");
            }
        }
        let result = task.await.unwrap();
        if step == 2 {
            let _ = result.expect_err("replay dispatch rejected");
        } else {
            result.unwrap();
        }
    }
    assert!(attempt.accepted());
    c.revoke().unwrap();
}
#[tokio::test]
async fn pairing_sdk_total_deadline_bounds_stalled_partial_frames() {
    for operation in ["inspect", "confirm", "cancel"] {
        for partial in [false, true] {
            let (endpoint, mut pipe) = isolated_pair_pipe();
            let task = tokio::spawn(async move {
                pair_pipe_hello(&mut pipe).await;
                let _: crate::protocol::Request =
                    crate::protocol::wire::read(&mut pipe).await.unwrap();
                if operation == "confirm" {
                    let _: crate::protocol::pairing_comparison::PairingCode =
                        crate::protocol::wire::read(&mut pipe).await.unwrap();
                }
                if partial {
                    pipe.write_all(&[0, 0]).await.unwrap();
                }
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            });
            let client = crate::sdk::Client::new(None)
                .unwrap()
                .with_endpoint(endpoint)
                .with_deadlines(
                    std::time::Duration::from_secs(1),
                    std::time::Duration::from_millis(80),
                );
            let id = uuid::Uuid::new_v4().to_string();
            let start = Instant::now();
            let code = crate::protocol::pairing_comparison::PairingCode {
                comparison: "ABCD 1234 DCBA 5678".into(),
            };
            match operation {
                "inspect" => {
                    let _ = client.pairing_inspect(&id).await.expect_err("deadline");
                }
                "confirm" => {
                    let _ = client
                        .pairing_confirm(&id, &code)
                        .await
                        .expect_err("deadline");
                }
                _ => {
                    let _ = client.pairing_cancel(&id).await.expect_err("deadline");
                }
            }
            assert!(start.elapsed() < std::time::Duration::from_secs(1));
            task.abort();
        }
    }
}
#[tokio::test]
async fn pairing_sdk_cancellation_drops_partial_exchange_pipe() {
    let (endpoint, mut pipe) = isolated_pair_pipe();
    let (seen, wait) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        pair_pipe_hello(&mut pipe).await;
        let _: crate::protocol::Request = crate::protocol::wire::read(&mut pipe).await.unwrap();
        let _ = seen.send(());
        let mut byte = [0];
        assert_eq!(
            tokio::time::timeout(std::time::Duration::from_secs(1), pipe.read(&mut byte))
                .await
                .unwrap()
                .unwrap(),
            0
        );
    });
    let client = crate::sdk::Client::new(None)
        .unwrap()
        .with_endpoint(endpoint);
    let caller = tokio::spawn(async move {
        client
            .pairing_inspect(&uuid::Uuid::new_v4().to_string())
            .await
    });
    wait.await.unwrap();
    caller.abort();
    let _ = caller.await;
    server.await.unwrap();
}

#[test]
fn safe_pairing_diagnostics_have_stage_and_category_without_code() {
    let f = Fixture::new();
    let c = f.controller();
    c.arm().unwrap();
    let (_, _, a) = c.capture_pairing().unwrap();
    a.track(Arc::downgrade(&c));
    a.stage(crate::protocol::phone_status::PhonePhase::PairingComparison);
    a.failure(crate::protocol::phone_status::PhoneFailureCategory::ReadClosed);
    let s = c.status().unwrap();
    assert_eq!(
        s.diagnostic.phase,
        crate::protocol::phone_status::PhonePhase::PairingComparison
    );
    assert_eq!(
        s.diagnostic.last_failure.unwrap().category,
        crate::protocol::phone_status::PhoneFailureCategory::ReadClosed
    );
    assert!(!format!("{s:?}").contains("ABCD"));
    c.revoke().unwrap();
}

#[test]
fn pairing_transport_diagnostic_keeps_only_numeric_kind_and_elapsed() {
    let f = Fixture::new();
    let c = f.controller();
    c.phase(crate::protocol::phone_status::PhonePhase::PairingComparison);
    c.transport_failure(&std::io::Error::from_raw_os_error(10054));
    let d = c.status().unwrap().diagnostic;
    let failure = d.last_failure.unwrap();
    assert_eq!(failure.transport_os_code, Some(10054));
    assert_eq!(
        failure.transport_kind,
        Some(crate::protocol::phone_status::PhoneTransportKind::ConnectionReset)
    );
    assert!(failure.elapsed_phase_ms.is_some());
    assert!(d.phase_started_unix_ms.is_some());
    let secret = "secret-session-code";
    c.transport_failure(&std::io::Error::new(
        std::io::ErrorKind::ConnectionAborted,
        secret,
    ));
    assert!(!format!("{:?}", c.status().unwrap()).contains(secret));
}
