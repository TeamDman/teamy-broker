use super::Client;
impl Client {
    /// # Errors
    /// Rejects invalid metadata, unavailable state, cancellation or expiry.
    pub async fn phone_authorize_once(
        &self,
        request: crate::protocol::Request,
    ) -> eyre::Result<crate::protocol::PhoneStatus> {
        request.validate()?;
        eyre::ensure!(
            matches!(
                request.operation.as_str(),
                "phone-authorize-desktop" | "phone-authorize-supply" | "phone-authorize-fixture"
            ),
            "permission operation rejected"
        );
        tokio::time::timeout(
            self.request_timeout.min(std::time::Duration::from_secs(5)),
            async {
                let mut pipe = self
                    .connect(false, Some("persistent-no-vault-phone-v1"))
                    .await?;
                crate::protocol::wire::write(&mut pipe, &request).await?;
                let reply: crate::protocol::Response =
                    crate::protocol::wire::read(&mut pipe).await?;
                eyre::ensure!(
                    reply.code == "phone-status",
                    "per-action permission rejected"
                );
                crate::protocol::wire::read(&mut pipe).await
            },
        )
        .await
        .map_err(|_redacted_error| eyre::eyre!("phone exchange deadline"))?
    }
    /// Current-user secured pipe; no autostart, no certificate/key export.
    /// # Errors
    /// Rejects unavailable/legacy residents or invalid relationship controls.
    pub async fn phone_control(
        &self,
        operation: &str,
    ) -> eyre::Result<crate::protocol::PhoneStatus> {
        eyre::ensure!(
            matches!(
                operation,
                "phone-pair" | "phone-revoke" | "phone-list" | "phone-status"
            ),
            "phone control rejected"
        );
        let request = crate::protocol::Request::control(operation, None);
        request.validate()?;
        tokio::time::timeout(
            self.request_timeout.min(std::time::Duration::from_secs(5)),
            async {
                let mut pipe = self
                    .connect(false, Some("persistent-no-vault-phone-v1"))
                    .await?;
                crate::protocol::wire::write(&mut pipe, &request).await?;
                let reply: crate::protocol::Response =
                    crate::protocol::wire::read(&mut pipe).await?;
                eyre::ensure!(
                    reply.code == "phone-status",
                    "phone control rejected; inspect status and revoke before replacing"
                );
                crate::protocol::wire::read(&mut pipe).await
            },
        )
        .await
        .map_err(|_redacted_error| eyre::eyre!("phone exchange deadline"))?
    }
}

impl Client {
    async fn pairing_pipe(
        &self,
        operation: &str,
        id: &str,
    ) -> eyre::Result<tokio::net::windows::named_pipe::NamedPipeClient> {
        let request = crate::protocol::Request::control(operation, Some(id.into()));
        request.validate()?;
        let mut pipe = self
            .connect(false, Some("persistent-no-vault-phone-v1"))
            .await?;
        crate::protocol::wire::write(&mut pipe, &request).await?;
        Ok(pipe)
    }
    /// # Errors
    /// Requires exact live attempt; deliberately reveals only its ephemeral comparison.
    pub async fn pairing_inspect(
        &self,
        id: &str,
    ) -> eyre::Result<crate::protocol::pairing_comparison::PairingInspection> {
        tokio::time::timeout(
            self.request_timeout.min(std::time::Duration::from_secs(5)),
            async {
                let mut p = self.pairing_pipe("phone-pair-inspect", id).await?;
                let r: crate::protocol::Response = crate::protocol::wire::read(&mut p).await?;
                eyre::ensure!(r.code == "pairing-comparison", "comparison unavailable");
                crate::protocol::wire::read(&mut p).await
            },
        )
        .await
        .map_err(|_redacted_error| eyre::eyre!("pairing exchange deadline"))?
    }
    /// # Errors
    /// Requires exact live attempt and exact peer comparison; no latest/wildcard approval.
    pub async fn pairing_confirm(
        &self,
        id: &str,
        code: &crate::protocol::pairing_comparison::PairingCode,
    ) -> eyre::Result<crate::protocol::pairing_comparison::PairingConfirmationResult> {
        tokio::time::timeout(
            self.request_timeout.min(std::time::Duration::from_secs(5)),
            async {
                let mut p = self.pairing_pipe("phone-pair-confirm", id).await?;
                crate::protocol::wire::write(&mut p, code).await?;
                let r: crate::protocol::Response = crate::protocol::wire::read(&mut p).await?;
                eyre::ensure!(r.code == "pairing-confirmation", "confirmation unavailable");
                crate::protocol::wire::read(&mut p).await
            },
        )
        .await
        .map_err(|_redacted_error| eyre::eyre!("pairing exchange deadline"))?
    }
    /// # Errors
    /// Cancels only the exact live comparison, without revoking stored trust.
    pub async fn pairing_cancel(
        &self,
        id: &str,
    ) -> eyre::Result<crate::protocol::pairing_comparison::PairingConfirmationResult> {
        tokio::time::timeout(
            self.request_timeout.min(std::time::Duration::from_secs(5)),
            async {
                let mut p = self.pairing_pipe("phone-pair-cancel", id).await?;
                let r: crate::protocol::Response = crate::protocol::wire::read(&mut p).await?;
                eyre::ensure!(r.code == "pairing-confirmation", "cancellation unavailable");
                crate::protocol::wire::read(&mut p).await
            },
        )
        .await
        .map_err(|_redacted_error| eyre::eyre!("pairing exchange deadline"))?
    }
}
