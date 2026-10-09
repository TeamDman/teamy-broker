use super::adapter::Adapter;
use crate::protocol::AcquireRequest;
use crate::protocol::MAX_SECRET;
use std::pin::Pin;
use std::process::Stdio;
use tokio::io::AsyncReadExt;
use zeroize::Zeroizing;
#[derive(Debug)]
pub struct OnePassword {
    pub executable: std::path::PathBuf,
}
impl Adapter for OnePassword {
    fn read<'a>(
        &'a self,
        request: &'a AcquireRequest,
    ) -> Pin<Box<dyn Future<Output = eyre::Result<Zeroizing<Vec<u8>>>> + Send + 'a>> {
        Box::pin(async move {
            request.validate()?;
            // Exactly op read, explicit account, no shells, command proxy, service-account or Connect fallback.
            let mut command = tokio::process::Command::new(&self.executable);
            command
                .args([
                    "read",
                    "--account",
                    &request.account,
                    "--no-newline",
                    &request.secret_ref,
                ])
                .env_remove("OP_SERVICE_ACCOUNT_TOKEN")
                .env_remove("OP_CONNECT_TOKEN")
                .env_remove("OP_CONNECT_HOST")
                .env_remove("OP_SESSION")
                .env("OP_BIOMETRIC_UNLOCK_ENABLED", "true")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .kill_on_drop(true);
            for (key, _value) in std::env::vars_os() {
                if key.to_string_lossy().starts_with("OP_SESSION_") {
                    command.env_remove(key);
                }
            }
            #[cfg(windows)]
            command.creation_flags(0x0800_0000);
            let mut child = command
                .spawn()
                .map_err(|_redacted_error| eyre::eyre!("1Password invocation failed"))?;
            let mut stdout = child
                .stdout
                .take()
                .ok_or_else(|| eyre::eyre!("1Password stream unavailable"))?;
            let mut bytes = Zeroizing::new(Vec::new());
            let operation = async {
                (&mut stdout)
                    .take(u64::try_from(MAX_SECRET + 1)?)
                    .read_to_end(&mut bytes)
                    .await?;
                eyre::ensure!(bytes.len() <= MAX_SECRET, "1Password output too large");
                eyre::ensure!(
                    child.wait().await?.success(),
                    "1Password authorization or read failed"
                );
                Ok::<_, eyre::Report>(())
            };
            tokio::time::timeout(std::time::Duration::from_mins(2), operation)
                .await
                .map_err(|_redacted_error| eyre::eyre!("1Password timeout"))??;
            Ok(bytes)
        })
    }
}
