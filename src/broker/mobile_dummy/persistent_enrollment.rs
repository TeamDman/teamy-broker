use super::paired_socket::PairedSocket;
use super::paired_socket::read_frame;
use super::paired_socket::write_frame;
impl PairedSocket {
    /// UKEY2-authenticated public certificate enrollment after explicit Match on both devices.
    /// # Errors
    /// Rejects malformed certificates, failed durable publication, timeout or missing confirmation.
    pub async fn enroll_preview_identity(
        mut self,
        desktop: &[u8],
        publish: impl FnOnce(&[u8]) -> eyre::Result<()>,
    ) -> eyre::Result<()> {
        tokio::time::timeout_at(self.until, async {
            let frame = read_frame(&mut self.socket).await?;
            let certificate = self.worker.call(7, &frame).await?;
            eyre::ensure!(
                !certificate.is_empty() && certificate.len() <= 4096,
                "phone certificate rejected"
            );
            publish(&certificate)?;
            let encrypted = self.worker.call(6, desktop).await?;
            write_frame(&mut self.socket, &encrypted).await?;
            let frame = read_frame(&mut self.socket).await?;
            let reply = self.worker.call(7, &frame).await?;
            eyre::ensure!(
                reply.as_slice() == b"persistent-enrolled-v1",
                "phone enrollment rejected"
            );
            let final_ack = self.worker.call(6, b"persistent-enrolled-v1").await?;
            write_frame(&mut self.socket, &final_ack).await?;
            Ok::<_, eyre::Report>(())
        })
        .await?
    }
}
