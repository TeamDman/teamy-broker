use crate::broker::phone::ENROLLMENT_WINDOW;
use crate::broker::phone::SCOPE;
use crate::broker::phone::config::PhoneProfile;
use crate::paths::AppHome;
use std::env;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use teamy_broker_handoff::pairing_binding::PairingBinding;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tokio::process::Child;
use tokio::process::ChildStdin;
use tokio::process::ChildStdout;
use tokio::process::Command;
use tokio::time;
use tokio::time::Instant;
use zeroize::Zeroizing;
/// Resolve the explicitly selected JDK without searching an untrusted executable path.
/// # Errors
/// Rejects a missing, relative, or unavailable JDK executable.
pub fn configured_java() -> eyre::Result<PathBuf> {
    let path = PhoneProfile::load(&AppHome::resolve()?)?.java;
    path.require_available()?;
    Ok(path.into_path_buf())
}
/// Existing-JDK adapter around pinned official Java UKEY2. No subprocess output becomes a log.
pub struct JavaPairingWorker {
    child: Child,
    input: ChildStdin,
    output: ChildStdout,
    until: Instant,
}
impl Debug for JavaPairingWorker {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
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
        until: Instant,
    ) -> eyre::Result<Self> {
        Self::start_role(java, artifacts, binding, until, false).await
    }
    #[cfg(test)]
    pub(super) async fn start_test_phone(
        java: &Path,
        artifacts: &Path,
        binding: &PairingBinding,
        until: Instant,
    ) -> eyre::Result<Self> {
        Self::start_role(java, artifacts, binding, until, true).await
    }
    async fn start_role(
        java: &Path,
        artifacts: &Path,
        binding: &PairingBinding,
        until: Instant,
        phone: bool,
    ) -> eyre::Result<Self> {
        eyre::ensure!(
            java.is_file() && artifacts.join("build/ukey2-worker.jar").is_file(),
            "pairing runtime unavailable"
        );
        eyre::ensure!(
            until > Instant::now()
                && until
                    <= Instant::now()
                        + if binding.scope == SCOPE {
                            ENROLLMENT_WINDOW
                        } else {
                            Duration::from_mins(3)
                        },
            "finite pairing session required"
        );
        let classpath = env::join_paths([
            artifacts.join("build/ukey2-worker.jar"),
            artifacts.join("deps/*"),
        ])?;
        let mut command = Command::new(java);
        command
            .args(["-Xmx64m", "-cp"])
            .arg(classpath)
            .arg("dev.teamy.broker.pairing.PipeWorker")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        if binding.scope == SCOPE {
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
        let result = time::timeout_at(
            self.until.min(Instant::now() + Duration::from_secs(5)),
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

#[cfg(test)]
mod package_validation_tests {
    use super::*;
    use crate::broker::phone::config::JavaExecutablePath;
    use crate::broker::phone::pairing::pairing_runtime::PairingRuntime;
    use crate::protocol::wire;

    #[tokio::test]
    #[ignore = "package validation: copy this test executable beside a standalone broker package; no device, daemon or native UI"]
    async fn relocated_package_initializes_worker() -> eyre::Result<()> {
        let executable = env::current_exe()?;
        let directory = executable
            .parent()
            .ok_or_else(|| eyre::eyre!("package validation directory unavailable"))?;
        eyre::ensure!(
            directory.join("teamy-broker.exe").is_file()
                && directory.join("package-manifest.json").is_file(),
            "package validation requires a standalone package"
        );
        let artifacts = PairingRuntime::installed()?;
        let java = JavaExecutablePath::for_tests()?;
        let binding = PairingBinding::dummy(wire::unix_ms()?)?;
        let mut worker = JavaPairingWorker::start(
            java.as_path(),
            artifacts.as_path(),
            &binding,
            Instant::now() + Duration::from_secs(10),
        )
        .await?;
        // start waits for the actual worker's initialization ACK. No peer,
        // network connection, comparison code or credential is involved.
        worker
            .child
            .start_kill()
            .map_err(|_redacted| eyre::eyre!("package worker termination failed"))?;
        let _status = time::timeout(Duration::from_secs(3), worker.child.wait())
            .await
            .map_err(|_redacted| eyre::eyre!("package worker termination deadline"))?
            .map_err(|_redacted| eyre::eyre!("package worker wait failed"))?;
        Ok(())
    }
}
