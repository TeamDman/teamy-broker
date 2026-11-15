use super::Controller;
use super::SCOPE;
use super::message::Message;
use super::message::Reply;
use super::store::Record;
use crate::broker::mobile_dummy::phone_profile::PhoneProfile;
use crate::broker::mobile_dummy::tailnet_peer::TailnetPeer;
use crate::protocol::ApprovalPreviewDisposition;
use crate::protocol::phone_status::PhoneFailureCategory;
use crate::protocol::phone_status::PhonePhase;
use crate::protocol::wire;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::io::AsyncRead;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
#[cfg(test)]
use tokio::io::DuplexStream;
use tokio::net::TcpStream;
use tokio::time;
use tokio::time::Instant;
use tokio_rustls::server::TlsStream;
pub(super) async fn connection(
    socket: TcpStream,
    peer: SocketAddr,
    controller: Arc<Controller>,
    record: Record,
    profile: PhoneProfile,
) -> eyre::Result<()> {
    serve(socket, controller, record, RealPeer(peer, profile)).await
}
trait PeerCheck {
    async fn verify(&self) -> eyre::Result<()>;
}
struct RealPeer(SocketAddr, PhoneProfile);
impl PeerCheck for RealPeer {
    async fn verify(&self) -> eyre::Result<()> {
        TailnetPeer::verify_desktop_for(&self.1).await?;
        TailnetPeer::verify_peer_for(self.0, &self.1).await
    }
}
async fn serve<S: AsyncRead + AsyncWrite + Unpin>(
    socket: S,
    controller: Arc<Controller>,
    record: Record,
    peer: impl PeerCheck,
) -> eyre::Result<()> {
    controller.phase(PhonePhase::PeerVerification);
    peer.verify().await?;
    let mut tls = accept_tls(socket, &controller, &record).await?;
    controller.phase(PhonePhase::CertificateAuthorization);
    let certificate = tls
        .get_ref()
        .1
        .peer_certificates()
        .and_then(|chain| chain.first())
        .ok_or_else(|| eyre::eyre!("phone certificate required"))?
        .to_vec();
    eyre::ensure!(
        controller.authorized(&record.generation, &certificate)?,
        "phone revoked or replaced"
    );
    let session = uuid::Uuid::new_v4().to_string();
    let mut sequence = 0;
    controller.connected.store(true, Ordering::Release);
    let _guard = Connected(Arc::clone(&controller));
    let until = Instant::now() + Duration::from_mins(30);

    let hello = Reply {
        version: 1,
        scope: SCOPE.into(),
        session_id: session.clone(),
        sequence,
        pending: controller.snapshot(),
        accepted: None,
    };
    controller.phase(PhonePhase::HelloWrite);
    time::timeout(super::timing::WRITE, async {
        wire::write(&mut tls, &hello).await?;
        tls.flush().await?;
        Ok::<_, eyre::Report>(())
    })
    .await??;
    loop {
        eyre::ensure!(
            controller.authorized(&record.generation, &certificate)? && Instant::now() < until,
            "phone connection ended"
        );
        sequence = sequence
            .checked_add(1)
            .ok_or_else(|| eyre::eyre!("phone protocol rejected"))?;
        controller.phase(PhonePhase::MessageRead);
        let message = super::message_read::read(&mut tls, &controller, super::timing::IDLE).await?;
        controller.phase(PhonePhase::MessageValidation);
        message.validate(&session, sequence)?;
        controller.phase(PhonePhase::PeerReverification);
        peer.verify().await?;
        eyre::ensure!(
            controller.authorized(&record.generation, &certificate)?,
            "phone revoked or replaced"
        );
        let accepted = apply_message(
            &message,
            &mut tls,
            &controller,
            &record,
            &certificate,
            &peer,
        )
        .await?;
        let reply = Reply {
            version: 1,
            scope: SCOPE.into(),
            session_id: session.clone(),
            sequence,
            pending: controller.snapshot(),
            accepted,
        };
        controller.phase(PhonePhase::ReplyWrite);
        time::timeout(super::timing::WRITE, async {
            wire::write(&mut tls, &reply).await?;
            tls.flush().await?;
            Ok::<_, eyre::Report>(())
        })
        .await??;
    }
}
async fn apply_message<S: AsyncRead + AsyncWrite + Unpin>(
    message: &Message,
    tls: &mut TlsStream<S>,
    controller: &Controller,
    record: &Record,
    certificate: &[u8],
    peer: &impl PeerCheck,
) -> eyre::Result<Option<bool>> {
    let accepted = if message.operation == "supply" {
        let bytes = time::timeout(super::timing::WRITE, wire::read_secret(tls)).await??;
        peer.verify().await?;
        Some(
            controller.supply(
                &record.generation,
                certificate,
                message
                    .request_id
                    .as_deref()
                    .ok_or_else(|| eyre::eyre!("missing request ID"))?,
                bytes,
            )?,
        )
    } else if message.operation == "decide" {
        let decision = match message.decision.as_deref() {
            Some("approve") => ApprovalPreviewDisposition::ApprovedTest,
            Some("deny") => ApprovalPreviewDisposition::DeniedTest,
            Some("cancel") => ApprovalPreviewDisposition::Cancelled,
            _ => eyre::bail!("phone operation rejected"),
        };
        Some(
            controller.resolve(
                &record.generation,
                certificate,
                message
                    .request_id
                    .as_deref()
                    .ok_or_else(|| eyre::eyre!("missing preview ID"))?,
                decision,
            )?,
        )
    } else {
        None
    };
    Ok(accepted)
}
async fn accept_tls<S: AsyncRead + AsyncWrite + Unpin>(
    socket: S,
    controller: &Controller,
    record: &Record,
) -> eyre::Result<TlsStream<S>> {
    controller.phase(PhonePhase::TlsConfiguration);
    let acceptor = super::identity::acceptor(record)?;
    controller.phase(PhonePhase::TlsHandshake);
    let tls = match time::timeout(super::timing::WRITE, acceptor.accept(socket)).await {
        Ok(Ok(tls)) => tls,
        Ok(Err(error)) => {
            controller.failure(super::diagnostic::tls_category(&error));
            eyre::bail!("phone TLS rejected")
        }
        Err(_redacted) => {
            controller.failure(PhoneFailureCategory::Deadline);
            eyre::bail!("phone TLS deadline")
        }
    };
    Ok(tls)
}
#[derive(Debug)]
struct Connected(Arc<Controller>);
impl Drop for Connected {
    fn drop(&mut self) {
        self.0.connected.store(false, Ordering::Release);
    }
}

#[cfg(test)]
struct LoopbackPeer;
#[cfg(test)]
impl PeerCheck for LoopbackPeer {
    async fn verify(&self) -> eyre::Result<()> {
        Ok(())
    }
}
#[cfg(test)]
pub(super) async fn test_connection(
    socket: TcpStream,
    controller: Arc<Controller>,
    record: Record,
) -> eyre::Result<()> {
    eyre::ensure!(
        socket.peer_addr()?.ip().is_loopback(),
        "test loopback required"
    );
    serve(socket, controller, record, LoopbackPeer).await
}

#[cfg(test)]
struct SlowPeer;
#[cfg(test)]
impl PeerCheck for SlowPeer {
    async fn verify(&self) -> eyre::Result<()> {
        time::sleep(Duration::from_secs(3)).await;
        time::sleep(Duration::from_secs(3)).await;
        Ok(())
    }
}
#[cfg(test)]
pub(super) async fn test_slow_connection(
    socket: DuplexStream,
    controller: Arc<Controller>,
    record: Record,
) -> eyre::Result<()> {
    // Models the separate dispatch whois3s before serve's pair of3s identity checks.
    time::sleep(Duration::from_secs(3)).await;
    serve(socket, controller, record, SlowPeer).await
}
