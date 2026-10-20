use std::path::Path;
use std::process::Stdio;
use std::time::Duration;
use teamy_broker_handoff::pairing_binding::PairingBinding;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::process::Child;
use tokio::process::ChildStdin;
use tokio::process::ChildStdout;
use zeroize::Zeroizing;
/// Resolve the explicitly selected JDK without searching an untrusted executable path.
/// # Errors
/// Rejects a missing, relative, or unavailable JDK executable.
pub fn configured_java() -> eyre::Result<std::path::PathBuf> {
    let path = std::env::var_os("TEAMY_BROKER_JAVA")
        .map(std::path::PathBuf::from)
        .ok_or_else(|| eyre::eyre!("explicit Java executable required"))?;
    eyre::ensure!(
        path.is_absolute() && path.is_file(),
        "explicit Java executable unavailable"
    );
    Ok(path)
}
/// Existing-JDK adapter around pinned official Java UKEY2. No subprocess output becomes a log.
pub struct JavaPairingWorker {
    child: Child,
    input: ChildStdin,
    output: ChildStdout,
    until: tokio::time::Instant,
}
impl std::fmt::Debug for JavaPairingWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("JavaPairingWorker([redacted])")
    }
}
impl JavaPairingWorker {
    /// # Errors
    /// Rejects missing compiled artifacts, failed worker start/frames or bounded operation timeout.
    pub async fn start(
        java: &Path,
        artifacts: &Path,
        binding: &PairingBinding,
        until: tokio::time::Instant,
    ) -> eyre::Result<Self> {
        Self::start_role(java, artifacts, binding, until, false).await
    }
    #[cfg(test)]
    pub(super) async fn start_test_phone(
        java: &Path,
        artifacts: &Path,
        binding: &PairingBinding,
        until: tokio::time::Instant,
    ) -> eyre::Result<Self> {
        Self::start_role(java, artifacts, binding, until, true).await
    }
    async fn start_role(
        java: &Path,
        artifacts: &Path,
        binding: &PairingBinding,
        until: tokio::time::Instant,
        phone: bool,
    ) -> eyre::Result<Self> {
        eyre::ensure!(
            java.is_file() && artifacts.join("build/ukey2-worker.jar").is_file(),
            "pairing runtime unavailable"
        );
        eyre::ensure!(
            until > tokio::time::Instant::now()
                && until
                    <= tokio::time::Instant::now()
                        + if binding.scope == crate::broker::phone::SCOPE {
                            crate::broker::phone::ENROLLMENT_WINDOW
                        } else {
                            Duration::from_mins(3)
                        },
            "finite pairing session required"
        );
        let classpath = std::env::join_paths([
            artifacts.join("build/ukey2-worker.jar"),
            artifacts.join("deps/*"),
        ])?;
        let mut command = tokio::process::Command::new(java);
        command
            .args(["-Xmx64m", "-cp"])
            .arg(classpath)
            .arg("dev.teamy.broker.pairing.PipeWorker")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        if binding.scope == crate::broker::phone::SCOPE {
            command.arg("--persistent-preview");
        }
        if phone {
            command.arg("--test-phone");
        }
        #[cfg(windows)]
        command.creation_flags(0x0800_0000); // Hidden, same user token; no native toolchain or elevation.
        let mut child = command
            .spawn()
            .map_err(|_redacted_error| eyre::eyre!("pairing runtime unavailable"))?;
        let input = child
            .stdin
            .take()
            .ok_or_else(|| eyre::eyre!("pairing runtime unavailable"))?;
        let output = child
            .stdout
            .take()
            .ok_or_else(|| eyre::eyre!("pairing runtime unavailable"))?;
        let mut worker = Self {
            child,
            input,
            output,
            until,
        };
        worker.call(0, binding.encode()?.as_bytes()).await?;
        Ok(worker)
    }
    /// Private anonymous pipe request. Payload/result buffers clear on drop best effort.
    /// # Errors
    /// Returns only a static redacted error; caller must terminate the live socket on error.
    pub async fn call(
        &mut self,
        operation: u8,
        payload: &[u8],
    ) -> eyre::Result<Zeroizing<Vec<u8>>> {
        eyre::ensure!(
            operation <= 7 && payload.len() < 16_384,
            "pairing operation rejected"
        );
        let result = tokio::time::timeout_at(
            self.until
                .min(tokio::time::Instant::now() + Duration::from_secs(5)),
            async {
                let size = u32::try_from(payload.len() + 1)?;
                self.input.write_u32(size).await?;
                self.input.write_u8(operation).await?;
                self.input.write_all(payload).await?;
                self.input.flush().await?;
                let size = self.output.read_u32().await?;
                eyre::ensure!((1..=16_384).contains(&size), "pairing frame rejected");
                let status = self.output.read_u8().await?;
                eyre::ensure!(status == 0, "pairing rejected");
                let mut bytes = Zeroizing::new(vec![0; usize::try_from(size - 1)?]);
                self.output.read_exact(&mut bytes).await?;
                Ok::<_, eyre::Report>(bytes)
            },
        )
        .await;
        if let Ok(Ok(bytes)) = result {
            Ok(bytes)
        } else {
            let _ = self.child.start_kill();
            Err(eyre::eyre!("pairing unavailable"))
        }
    }
    /// # Errors
    /// Rejects unexpected display output; code appears only in native UI or explicitly scoped active-attempt inspection.
    pub async fn authentication_code(&mut self) -> eyre::Result<Zeroizing<String>> {
        let bytes = self.call(3, &[]).await?;
        eyre::ensure!(
            bytes.len() == 19
                && bytes.iter().enumerate().all(|(index, byte)| {
                    if index % 5 == 4 {
                        *byte == b' '
                    } else {
                        byte.is_ascii_hexdigit() && !byte.is_ascii_lowercase()
                    }
                }),
            "pairing code rejected"
        );
        Ok(Zeroizing::new(String::from_utf8(bytes.to_vec()).map_err(
            |_redacted_error| eyre::eyre!("pairing code rejected"),
        )?))
    }
}
