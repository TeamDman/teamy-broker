use super::packet::Packet;
use super::paired_socket::PairedSocket;
use super::paired_socket::read_frame;
use super::paired_socket::write_frame;
use super::preview_phase::PreviewMetadata;
use super::preview_phase::PreviewOrigin;
use super::preview_phase::PreviewPhase;
use super::preview_phase::PreviewTrace;
use super::preview_phase::emit;
use crate::protocol::ApprovalPreviewDisposition;
use crate::protocol::SharedPreviewSnapshot;
use crate::sdk::Client;
use std::sync::Arc;
use teamy_broker_handoff::envelope::Envelope;
use teamy_broker_handoff::pending::Pending;
use teamy_broker_handoff::session::Session;
use zeroize::Zeroizing;
#[derive(Debug)]
enum Observation {
    Poll(Option<SharedPreviewSnapshot>),
    Decision {
        accepted: bool,
        disposition: ApprovalPreviewDisposition,
    },
}
impl PairedSocket {
    /// Authenticated temporary phone bridge to the actual resident preview; never acquires.
    /// # Errors
    /// Closes on invalid frames, missing capability, disconnect or fixed session deadline.
    pub async fn serve_preview(self, session: Session, client: Client) -> eyre::Result<()> {
        self.serve_preview_traced(session, client, None).await
    }
    /// Optional bounded diagnostics contain fixed labels and typed primitive metadata only.
    /// # Errors
    /// Rejects protocol errors, expiry, disconnect or unavailable diagnostic output.
    pub async fn serve_preview_traced(
        mut self,
        session: Session,
        client: Client,
        trace: Option<Arc<PreviewTrace>>,
    ) -> eyre::Result<()> {
        let mut published = None;
        loop {
            eyre::ensure!(
                trace.as_ref().is_none_or(|trace| !trace.failed()),
                "phase diagnostics unavailable"
            );
            let frame =
                match tokio::time::timeout_at(self.until, read_frame(&mut self.socket)).await {
                    Ok(Ok(frame)) => frame,
                    Ok(Err(_redacted_error)) => {
                        emit(trace.as_deref(), PreviewPhase::TransportUnavailable);
                        eyre::bail!("preview transport unavailable");
                    }
                    Err(_redacted_error) => {
                        emit(trace.as_deref(), PreviewPhase::SessionDeadline);
                        eyre::bail!("preview session expired");
                    }
                };
            let until = self
                .until
                .min(tokio::time::Instant::now() + std::time::Duration::from_secs(5));
            tokio::time::timeout_at(until, async {
                let plain = self.worker.call(7, &frame).await?;
                let packet: Packet = facet_json::from_str(std::str::from_utf8(&plain)?)?;
                let (response, observation) = process(&session, &client, packet).await?;
                let response = Zeroizing::new(facet_json::to_string(&response)?);
                let encrypted = self.worker.call(6, response.as_bytes()).await?;
                write_frame(&mut self.socket, &encrypted).await?;
                observe(trace.as_deref(), &mut published, observation);
                Ok::<_, eyre::Report>(())
            })
            .await??;
        }
    }
}
async fn process(
    session: &Session,
    client: &Client,
    packet: Packet,
) -> eyre::Result<(Envelope, Observation)> {
    match packet.operation.as_str() {
        "poll" => {
            let challenge = session.open(teamy_broker_handoff::POLL_AAD, &packet.envelope)?;
            eyre::ensure!(challenge.len() == 32, "invalid preview poll");
            let state = client.shared_preview_state().await?;
            let snapshot = state.pending.clone();
            let pending: Vec<Pending> = state
                .pending
                .into_iter()
                .map(|snapshot| {
                    Ok(Pending {
                        binding: facet_json::to_string(&snapshot)?,
                    })
                })
                .collect::<eyre::Result<_>>()?;
            Ok((
                session.seal(&challenge, facet_json::to_string(&pending)?.as_bytes())?,
                Observation::Poll(snapshot),
            ))
        }
        "submit" => {
            let outer = session.open(teamy_broker_handoff::SUBMIT_AAD, &packet.envelope)?;
            let split = outer
                .iter()
                .position(|byte| *byte == 0)
                .ok_or_else(|| eyre::eyre!("invalid preview decision"))?;
            let exact = std::str::from_utf8(&outer[..split])?;
            let snapshot: SharedPreviewSnapshot = facet_json::from_str(exact)?;
            let inner: Envelope = facet_json::from_str(std::str::from_utf8(&outer[split + 1..])?)?;
            let decision = session.open(exact.as_bytes(), &inner)?;
            let disposition = match decision.as_slice() {
                [1] => ApprovalPreviewDisposition::ApprovedTest,
                [0] => ApprovalPreviewDisposition::DeniedTest,
                [2] => ApprovalPreviewDisposition::Cancelled,
                _ => eyre::bail!("invalid preview decision"),
            };
            let state = client.shared_preview_state().await?;
            let accepted = state.pending.as_ref() == Some(&snapshot)
                && client
                    .shared_preview_decide(&snapshot.request_id, disposition)
                    .await?;
            Ok((
                session.seal(
                    inner.nonce.as_bytes(),
                    if accepted { b"accepted" } else { b"stale" },
                )?,
                Observation::Decision {
                    accepted,
                    disposition,
                },
            ))
        }
        _ => eyre::bail!("invalid preview packet"),
    }
}
fn observe(
    trace: Option<&PreviewTrace>,
    published: &mut Option<SharedPreviewSnapshot>,
    observation: Observation,
) {
    match observation {
        Observation::Poll(snapshot) => {
            if snapshot != *published {
                let phase = if snapshot.is_some() {
                    PreviewPhase::PreviewPublished
                } else {
                    PreviewPhase::PreviewWithdrawn
                };
                if let Some(trace) = trace {
                    trace.emit(
                        phase,
                        PreviewMetadata {
                            ack_written: Some(true),
                            origin: Some(PreviewOrigin::Broker),
                            ..Default::default()
                        },
                    );
                }
                *published = snapshot;
            }
        }
        Observation::Decision {
            accepted,
            disposition,
        } => {
            if let Some(trace) = trace {
                trace.emit(
                    if accepted {
                        PreviewPhase::PreviewResolved
                    } else {
                        PreviewPhase::PreviewStale
                    },
                    PreviewMetadata {
                        accepted: Some(accepted),
                        ack_written: Some(true),
                        origin: Some(PreviewOrigin::Phone),
                        decision: Some(disposition),
                        ..Default::default()
                    },
                );
            }
        }
    }
}
