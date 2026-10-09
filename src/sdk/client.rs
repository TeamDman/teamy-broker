use crate::protocol::Hello;
use crate::protocol::Request;
use crate::protocol::Response;
use crate::protocol::{self};
use std::os::windows::process::CommandExt;
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
    request_timeout: Duration,
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
    async fn connect(&self, start: bool) -> eyre::Result<NamedPipeClient> {
        tokio::time::timeout(self.readiness, async {
            let mut attempted = false;
            loop {
                match ClientOptions::new().open(&self.endpoint) {
                    Ok(mut pipe) => {
                        let hello: Hello = protocol::wire::read(&mut pipe).await?;
                        eyre::ensure!(
                            hello.compatible(),
                            "incompatible broker; refusing autostart"
                        );
                        let peer = Hello {
                            version: protocol::VERSION,
                            capabilities: protocol::CAPABILITIES
                                .iter()
                                .map(|c| (*c).into())
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
                            let mut command = std::process::Command::new(exe);
                            command
                                .args(&self.start_arguments)
                                .stdin(std::process::Stdio::null())
                                .stdout(std::process::Stdio::null())
                                .stderr(std::process::Stdio::null())
                                .creation_flags(0x0800_0000);
                            command
                                .spawn()
                                .map_err(|_redacted_error| eyre::eyre!("broker start failed"))?;
                            attempted = true;
                        }
                        if !start && e.raw_os_error() == Some(2) {
                            eyre::bail!("broker unavailable")
                        }
                        tokio::time::sleep(Duration::from_millis(25)).await;
                    }
                    Err(_redacted_error) => eyre::bail!("broker connection failed"),
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
        request.validate()?;
        let started = Instant::now();
        tokio::time::timeout(self.request_timeout, async {
            let mut pipe = self.connect(true).await?;
            protocol::wire::write(
                &mut pipe,
                &Request {
                    operation: "acquire".into(),
                    acquire: Some(request),
                    lease_id: None,
                },
            )
            .await?;
            let reply: Response = protocol::wire::read(&mut pipe).await?;
            eyre::ensure!(reply.code == "granted", "credential request denied");
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
        .map_err(|_redacted_error| eyre::eyre!("broker request timeout"))?
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
            let mut pipe = self.connect(false).await?;
            protocol::wire::write(&mut pipe, &request).await?;
            let response: Response = protocol::wire::read(&mut pipe).await?;
            eyre::ensure!(response.code == "ok", "broker control failed");
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
            Ok(response) => response.status.ok_or_else(|| eyre::eyre!("missing status")),
            Err(e) if e.to_string() == "broker unavailable" => Ok(protocol::Status::default()),
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
        self.control(Request::control("stop", None)).await?;
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
