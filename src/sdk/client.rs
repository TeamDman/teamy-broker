use crate::protocol::Hello;
use crate::protocol::Request;
use crate::protocol::Response;
use crate::protocol::{self};
use std::time::Duration;
use std::time::Instant;
use tokio::net::windows::named_pipe::ClientOptions;
use tokio::net::windows::named_pipe::NamedPipeClient;
#[derive(Clone)]
pub struct Client {
    endpoint: String,
    start_arguments: Vec<String>,
    broker_executable: Option<std::path::PathBuf>,
    readiness: Duration,
    pub(super) request_timeout: Duration,
}
impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Client([redacted configuration])")
    }
}
impl Client {
    /// Explicit executable enables autostart. None never spawns or falls back.
    /// # Errors
    ///
    /// Returns an error if the current user's identity cannot be resolved.
    pub fn new(broker_executable: Option<std::path::PathBuf>) -> eyre::Result<Self> {
        Ok(Self {
            endpoint: crate::platform::identity::endpoint()?,
            start_arguments: vec!["serve".into()],
            broker_executable,
            readiness: Duration::from_secs(10),
            request_timeout: Duration::from_secs(190),
        })
    }
    #[must_use]
    pub fn with_deadlines(mut self, readiness: Duration, request: Duration) -> Self {
        self.readiness = readiness;
        self.request_timeout = request;
        self
    }
    /// Explicit alternate endpoint for isolated synthetic tests.
    #[must_use]
    pub fn with_endpoint(mut self, endpoint: String) -> Self {
        self.endpoint = endpoint;
        self
    }
    /// Arguments to the explicitly selected local executable, never a broker-side proxy.
    #[must_use]
    pub fn with_start_arguments(mut self, arguments: Vec<String>) -> Self {
        self.start_arguments = arguments;
        self
    }
    pub(super) async fn connect(
        &self,
        start: bool,
        require_capability: Option<&str>,
    ) -> eyre::Result<NamedPipeClient> {
        tokio::time::timeout(self.readiness, async {
            let mut attempted = false;
            loop {
                match ClientOptions::new().open(&self.endpoint) {
                    Ok(mut pipe) => {
                        let hello: Hello = protocol::wire::read(&mut pipe)
                            .await
                            .map_err(|_redacted_error| eyre::eyre!("broker handshake failed"))?;
                        eyre::ensure!(
                            hello.compatible(),
                            "incompatible broker; refusing autostart"
                        );
                        eyre::ensure!(
                            require_capability.is_none()
                                || hello
                                    .capabilities
                                    .iter()
                                    .any(|cap| Some(cap.as_str()) == require_capability),
                            if require_capability == Some("shutdown-state-v1") {
                                "broker shutdown status unsupported; update broker explicitly"
                            } else {
                                "broker approval preview unsupported; update broker explicitly"
                            }
                        );
                        let peer = Hello {
                            version: protocol::VERSION,
                            capabilities: protocol::CAPABILITIES
                                .iter()
                                .copied()
                                .chain(require_capability)
                                .map(str::to_string)
                                .collect(),
                            instance_id: String::new(),
                        };
                        protocol::wire::write(&mut pipe, &peer).await?;
                        return Ok(pipe);
                    }
                    Err(e) if matches!(e.raw_os_error(), Some(2 | 231)) => {
                        if e.raw_os_error() == Some(2) && start && !attempted {
                            let exe = self.broker_executable.as_ref().ok_or_else(|| {
                                eyre::eyre!("broker unavailable; autostart not configured")
                            })?;
                            crate::platform::detached_process::spawn(exe, &self.start_arguments)?;
                            attempted = true;
                        }
                        if !start && e.raw_os_error() == Some(2) {
                            eyre::bail!("broker unavailable")
                        }
                        tokio::time::sleep(Duration::from_millis(25)).await;
                    }
                    Err(error) => return Err(connection_error(&error)),
                }
            }
        })
        .await
        .map_err(|_redacted_error| eyre::eyre!("broker readiness deadline exceeded"))?
    }
    /// Obtain an exact item through native preflight and the broker's fixed op-read adapter.
    /// # Errors
    ///
    /// Fails on invalid metadata, incompatible IPC, denial, timeout, or expired lease.
    pub async fn acquire(&self, request: protocol::AcquireRequest) -> eyre::Result<super::Secret> {
        self.acquire_mode(request, false).await
    }
    /// # Errors
    /// Uses the explicit fixture operation; fails closed on absent/expired/consumed grant.
    pub async fn acquire_fixture(
        &self,
        request: protocol::AcquireRequest,
    ) -> eyre::Result<super::Secret> {
        self.acquire_mode(request, true).await
    }
    async fn acquire_mode(
        &self,
        request: protocol::AcquireRequest,
        fixture: bool,
    ) -> eyre::Result<super::Secret> {
        request.validate()?;
        let started = Instant::now();
        tokio::time::timeout(self.request_timeout, async {
            let mut pipe = self
                .connect(
                    !fixture,
                    fixture.then_some("isolated-credential-fixture-v1"),
                )
                .await?;
            protocol::wire::write(
                &mut pipe,
                &Request {
                    operation: if fixture {
                        "acquire-fixture"
                    } else {
                        "acquire"
                    }
                    .into(),
                    acquire: Some(request),
                    lease_id: None,
                },
            )
            .await?;
            let reply: Response =
                protocol::wire::read(&mut pipe)
                    .await
                    .map_err(|_redacted_error| {
                        eyre::Report::new(protocol::FailureKind::RequestFailed)
                    })?;
            if reply.code != "granted" {
                return Err(reply
                    .failure
                    .unwrap_or(protocol::FailureKind::RequestFailed)
                    .into());
            }
            eyre::ensure!(reply.failure.is_none(), "invalid granted response");
            let bytes = protocol::wire::read_secret(&mut pipe).await?;
            super::Secret::new(
                bytes,
                reply
                    .expires_unix_ms
                    .ok_or_else(|| eyre::eyre!("missing lease deadline"))?,
                reply
                    .lease_id
                    .ok_or_else(|| eyre::eyre!("missing lease ID"))?,
                started,
                reply
                    .remaining_millis
                    .ok_or_else(|| eyre::eyre!("missing lease duration"))?,
            )
        })
        .await
        .map_err(|_redacted_error| eyre::Report::new(protocol::FailureKind::RequestTimedOut))?
    }
    /// Read only the selected persistent provider, with live lease validation on both sides of the read.
    /// No autostart, new item approval, or fallback is attempted.
    /// # Errors
    ///
    /// Fails closed on missing storage, stale generation, purge, broker restart, expiry or I/O failure.
    pub async fn load_cached(
        &self,
        backend: protocol::Backend,
        lease_id: String,
    ) -> eyre::Result<super::Secret> {
        backend.validate()?;
        eyre::ensure!(
            backend != protocol::Backend::Memory,
            "explicit persistent provider required"
        );
        let started = Instant::now();
        let first = self
            .control(Request::control("validate", Some(lease_id.clone())))
            .await?;
        let envelope = crate::platform::cache_reader::read(&backend)?
            .ok_or_else(|| eyre::eyre!("cache unavailable"))?;
        eyre::ensure!(
            envelope.len() >= 32 && &envelope[..8] == b"TEAMYB01",
            "invalid cache envelope"
        );
        eyre::ensure!(
            uuid::Uuid::from_slice(&envelope[8..24])?.to_string() == lease_id,
            "stale cache generation"
        );
        let expiry = u64::from_be_bytes(envelope[24..32].try_into()?);
        eyre::ensure!(
            first.expires_unix_ms == Some(expiry),
            "cache deadline mismatch"
        );
        let second = self
            .control(Request::control("validate", Some(lease_id.clone())))
            .await?;
        eyre::ensure!(
            second.expires_unix_ms == Some(expiry),
            "cache lease changed"
        );
        super::Secret::new(
            zeroize::Zeroizing::new(envelope[32..].to_vec()),
            expiry,
            lease_id,
            started,
            second
                .remaining_millis
                .ok_or_else(|| eyre::eyre!("missing lease duration"))?,
        )
    }
    async fn control(&self, request: Request) -> eyre::Result<Response> {
        request.validate()?;
        tokio::time::timeout(self.request_timeout, async {
            let mut pipe = self
                .connect(
                    false,
                    matches!(request.operation.as_str(), "stop" | "stop-force")
                        .then_some("shutdown-state-v1"),
                )
                .await?;
            protocol::wire::write(&mut pipe, &request).await?;
            let response: Response = protocol::wire::read(&mut pipe).await?;
            if response.code != "ok" {
                if let Some(failure) = response.failure {
                    return Err(failure.into());
                }
                eyre::bail!("broker control failed");
            }
            Ok(response)
        })
        .await
        .map_err(|_redacted_error| eyre::eyre!("broker control timeout"))?
    }
    /// Redacted counts; absent endpoint is stopped. No autostart.
    /// # Errors
    ///
    /// Returns errors for incompatible, inaccessible, or unresponsive brokers.
    pub async fn status(&self) -> eyre::Result<protocol::Status> {
        match self.control(Request::control("status", None)).await {
            Ok(response) => {
                let mut status = response
                    .status
                    .ok_or_else(|| eyre::eyre!("missing status"))?;
                if !status.shutdown_state_available {
                    status.safe_to_stop = false;
                    status.would_discard_cached_leases = status.active_leases != 0;
                    status.stop_reasons =
                        vec![protocol::stop_reason::StopReason::StatusUnavailable];
                }
                Ok(status)
            }
            Err(e) if e.to_string() == "broker unavailable" => Ok(protocol::Status::stopped()),
            Err(e) => Err(e),
        }
    }
    /// Purge a specific lease or all leases, without revoking caller copies or upstream tokens.
    /// # Errors
    ///
    /// Returns an error if the broker or persistent cleanup is unavailable.
    pub async fn purge(&self, id: Option<String>) -> eyre::Result<()> {
        self.control(Request::control("purge", id)).await?;
        Ok(())
    }
    /// Gracefully stop and wait for the endpoint to disappear.
    /// # Errors
    ///
    /// Returns an error if cleanup fails or the shutdown deadline is exceeded.
    pub async fn stop(&self) -> eyre::Result<()> {
        self.stop_operation("stop").await
    }
    /// Explicit maintenance override: interrupts current work and discards cached leases.
    /// # Errors
    /// Returns an error if cleanup fails or shutdown times out.
    pub async fn stop_force(&self) -> eyre::Result<()> {
        self.stop_operation("stop-force").await
    }
    async fn stop_operation(&self, operation: &str) -> eyre::Result<()> {
        self.control(Request::control(operation, None)).await?;
        let deadline = Instant::now() + self.readiness;
        loop {
            match ClientOptions::new().open(&self.endpoint) {
                Err(e) if e.raw_os_error() == Some(2) => return Ok(()),
                _ => {
                    eyre::ensure!(
                        Instant::now() < deadline,
                        "broker shutdown deadline exceeded"
                    );
                    tokio::time::sleep(Duration::from_millis(25)).await;
                }
            }
        }
    }
}

fn connection_error(error: &std::io::Error) -> eyre::Report {
    if error.raw_os_error() == Some(5) {
        eyre::eyre!("broker access denied")
    } else {
        eyre::eyre!("broker connection failed")
    }
}

#[cfg(test)]
mod connection_error_tests {
    #[test]
    fn access_denied_is_distinct_and_other_io_payloads_are_redacted() {
        assert_eq!(
            super::connection_error(&std::io::Error::from_raw_os_error(5)).to_string(),
            "broker access denied"
        );
        assert_eq!(
            super::connection_error(&std::io::Error::other("SYNTHETIC_PRIVATE_PATH")).to_string(),
            "broker connection failed"
        );
    }
}
