use super::packet::Packet;
use super::paired_socket::PairedSocket;
use super::paired_socket::read_frame;
use super::paired_socket::write_frame;
use std::str;
use std::sync::Arc;
use std::time::Duration;
use teamy_broker_handoff::queue::Queue;
use tokio::time;
use tokio::time::Instant;
use zeroize::Zeroizing;

impl PairedSocket {
    /// # Errors
    /// Returns static redacted errors on timeout, invalid frames, authentication or queue rejection.
    pub(super) async fn serve(mut self, queue: Arc<Queue>) -> eyre::Result<()> {
        loop {
            let encrypted = time::timeout_at(self.until, read_frame(&mut self.socket))
                .await
                .map_err(|_redacted_error| eyre::eyre!("session expired"))??;
            let operation_until = self.until.min(Instant::now() + Duration::from_secs(5));
            time::timeout_at(operation_until, async {
                let plaintext = self.worker.call(7, &encrypted).await?;
                let text = str::from_utf8(&plaintext)
                    .map_err(|_redacted_error| eyre::eyre!("packet rejected"))?;
                let packet: Packet = facet_json::from_str(text)
                    .map_err(|_redacted_error| eyre::eyre!("packet rejected"))?;
                let envelope = match packet.operation.as_str() {
                    "poll" => queue.poll(&packet.envelope)?,
                    "submit" => queue.submit(&packet.envelope)?,
                    _ => eyre::bail!("packet rejected"),
                };
                let response = Zeroizing::new(
                    facet_json::to_string(&envelope)
                        .map_err(|_redacted_error| eyre::eyre!("packet rejected"))?,
                );
                let frame = self.worker.call(6, response.as_bytes()).await?;
                write_frame(&mut self.socket, &frame).await
            })
            .await
            .map_err(|_redacted_error| eyre::eyre!("session expired"))??;
        }
    }
}
