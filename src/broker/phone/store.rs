use facet::Facet;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use zeroize::Zeroizing;
#[derive(Facet, Clone)]
pub(super) struct Record {
    pub version: u32,
    pub scope: String,
    pub generation: String,
    pub certificate: Vec<u8>,
    pub private_key: Vec<u8>,
    pub phone_certificate: Option<Vec<u8>>,
}
impl Drop for Record {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.private_key);
    }
}
impl std::fmt::Debug for Record {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PhoneRecord([protected identity])")
    }
}
#[derive(Debug)]
pub(super) struct Store(pub PathBuf);
impl Store {
    pub fn current_user() -> eyre::Result<Self> {
        let path = directories_next::BaseDirs::new()
            .ok_or_else(|| eyre::eyre!("phone store unavailable"))?
            .data_local_dir()
            .join("teamy-broker")
            .join("phone-approvals-v1.dpapi");
        Ok(Self(path))
    }
    pub fn load(&self) -> eyre::Result<Option<Record>> {
        let file = match std::fs::File::open(&self.0) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_redacted_error) => eyre::bail!("phone store unavailable"),
        };
        let mut bytes = Vec::new();
        file.take(32769).read_to_end(&mut bytes)?;
        eyre::ensure!(bytes.len() <= 32768, "phone store rejected");
        let plain = crate::platform::dpapi::protect(&bytes, true)?;
        let record: Record = facet_json::from_str(
            std::str::from_utf8(&plain)
                .map_err(|_redacted_error| eyre::eyre!("phone identity rejected"))?,
        )
        .map_err(|_redacted_error| eyre::eyre!("phone identity rejected"))?;
        record.validate()?;
        Ok(Some(record))
    }
    pub fn save(&self, record: &Record) -> eyre::Result<()> {
        record.validate()?;
        let parent = self
            .0
            .parent()
            .ok_or_else(|| eyre::eyre!("phone store unavailable"))?;
        std::fs::create_dir_all(parent)?;
        let plain = Zeroizing::new(facet_json::to_string(record)?);
        let encrypted = crate::platform::dpapi::protect(plain.as_bytes(), false)?;
        let temporary = parent.join(format!("phone-{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(&encrypted)?;
            file.sync_all()?;
            drop(file);
            replace(&temporary, &self.0)
        })();
        let _ = std::fs::remove_file(&temporary);
        result
    }
}
fn replace(source: &Path, target: &Path) -> eyre::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::MOVEFILE_REPLACE_EXISTING;
    use windows_sys::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH;
    use windows_sys::Win32::Storage::FileSystem::MoveFileExW;
    let source: Vec<_> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let target: Vec<_> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: both NUL terminated paths remain alive throughout the synchronous atomic replacement.
    eyre::ensure!(
        // SAFETY: both path buffers are live and NUL terminated.
        unsafe {
            MoveFileExW(
                source.as_ptr(),
                target.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } != 0,
        "phone store publication failed"
    );
    Ok(())
}
impl Record {
    pub fn validate(&self) -> eyre::Result<()> {
        eyre::ensure!(
            self.version == 1
                && self.scope == super::SCOPE
                && uuid::Uuid::parse_str(&self.generation).is_ok()
                && !self.certificate.is_empty()
                && self.certificate.len() <= 4096
                && !self.private_key.is_empty()
                && self.private_key.len() <= 4096
                && self
                    .phone_certificate
                    .as_ref()
                    .is_none_or(|cert| !cert.is_empty() && cert.len() <= 4096),
            "phone identity rejected"
        );
        Ok(())
    }
}
