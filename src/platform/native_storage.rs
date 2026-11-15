use super::dpapi::protect;
use crate::protocol::Backend;
use std::fs::OpenOptions;
use std::fs::remove_file;
use std::io;
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::path::PathBuf;
use uuid::Uuid;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Security::Credentials::CRED_PERSIST_LOCAL_MACHINE;
use windows_sys::Win32::Security::Credentials::CRED_TYPE_GENERIC;
use windows_sys::Win32::Security::Credentials::CREDENTIALW;
use windows_sys::Win32::Security::Credentials::CredDeleteW;
use windows_sys::Win32::Security::Credentials::CredWriteW;
use windows_sys::Win32::Storage::FileSystem::MOVEFILE_REPLACE_EXISTING;
use windows_sys::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH;
use windows_sys::Win32::Storage::FileSystem::MoveFileExW;
/// # Errors
///
/// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
pub fn write(backend: &Backend, data: &[u8], generation: Uuid) -> eyre::Result<()> {
    match backend {
        Backend::Memory => Ok(()),
        Backend::WindowsCredentialManager(target) => {
            eyre::ensure!(data.len() <= 2560, "credential blob too large");
            let name = super::identity::wide(target);
            let credential = CREDENTIALW {
                Type: CRED_TYPE_GENERIC,
                TargetName: name.as_ptr().cast_mut(),
                CredentialBlobSize: u32::try_from(data.len())?,
                CredentialBlob: data.as_ptr().cast_mut(),
                Persist: CRED_PERSIST_LOCAL_MACHINE,
                ..Default::default()
            };
            // SAFETY: synchronous CredWrite copies pointers backed by live input buffers.
            eyre::ensure!(
                // SAFETY: synchronous CredWrite copies pointers backed by live input buffers.
                unsafe { CredWriteW(&raw const credential, 0) } != 0,
                "credential write failed"
            );
            Ok(())
        }
        Backend::WellKnownFile(path) => {
            let encrypted = protect(data, false)?;
            publish_file(Path::new(path), generation, &encrypted, replace_file)
        }
    }
}

fn staging_file(destination: &Path, generation: Uuid) -> eyre::Result<PathBuf> {
    let parent = destination
        .parent()
        .ok_or_else(|| eyre::eyre!("cache parent missing"))?;
    Ok(parent.join(format!(".teamy-broker-stage-{generation}.tmp")))
}

/// Reject a staging collision before publishing cleanup ownership for this generation.
pub(crate) fn require_staging_available(backend: &Backend, generation: Uuid) -> eyre::Result<()> {
    if let Backend::WellKnownFile(path) = backend {
        eyre::ensure!(
            !staging_file(Path::new(path), generation)?.try_exists()?,
            "cache staging collision"
        );
    }
    Ok(())
}

/// Remove only this descriptor's generation, including a partially written encrypted stage.
/// The distinct staging namespace never targets the older random temporary-file names.
pub(crate) fn remove_staging(backend: &Backend, generation: Uuid) -> eyre::Result<()> {
    if let Backend::WellKnownFile(path) = backend {
        match remove_file(staging_file(Path::new(path), generation)?) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => eyre::bail!("cache staging removal failed"),
        }
    }
    Ok(())
}

fn publish_file(
    destination: &Path,
    generation: Uuid,
    encrypted: &[u8],
    publish: impl FnOnce(&Path, &Path) -> eyre::Result<()>,
) -> eyre::Result<()> {
    let staging = staging_file(destination, generation)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staging)?;
    file.write_all(encrypted)?;
    file.sync_all()?;
    drop(file);
    // The durable descriptor owns this path before creation. Failed/interrupted publication
    // leaves it for generation-specific rollback or scheduled/startup cleanup.
    publish(&staging, destination)
}

fn replace_file(source: &Path, destination: &Path) -> eyre::Result<()> {
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let destination: Vec<u16> = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect();
    eyre::ensure!(
        // SAFETY: live NUL-terminated paths; the stage and destination share one directory.
        unsafe {
            MoveFileExW(
                source.as_ptr(),
                destination.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        } != 0,
        "cache publish failed"
    );
    Ok(())
}
/// # Errors
///
/// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
pub fn delete(backend: &Backend) -> eyre::Result<()> {
    match backend {
        Backend::Memory => Ok(()),
        Backend::WellKnownFile(path) => match remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(_) => eyre::bail!("cache removal failed"),
        },
        Backend::WindowsCredentialManager(target) => {
            let name = super::identity::wide(target);
            // SAFETY: valid NUL-terminated credential target.
            if unsafe { CredDeleteW(name.as_ptr(), CRED_TYPE_GENERIC, 0) } == 0 {
                // SAFETY: inspect error immediately following the failing Win32 call.
                eyre::ensure!(
                    // SAFETY: inspect error immediately following the failing Win32 call.
                    unsafe { GetLastError() } == 1168,
                    "credential removal failed"
                );
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;

    struct StagingFixture {
        directory: PathBuf,
        generation: Uuid,
        replacement: Uuid,
    }

    impl StagingFixture {
        fn new() -> Self {
            let directory =
                env::temp_dir().join(format!("teamy-broker-stage-test-{}", Uuid::new_v4()));
            fs::create_dir(&directory).unwrap();
            Self {
                directory,
                generation: Uuid::new_v4(),
                replacement: Uuid::new_v4(),
            }
        }
        fn destination(&self) -> PathBuf {
            self.directory.join("cache")
        }
        fn backend(&self) -> Backend {
            Backend::WellKnownFile(self.destination().to_str().unwrap().to_owned())
        }
        fn legacy(&self) -> PathBuf {
            self.directory
                .join(format!(".teamy-broker-{}.tmp", self.generation))
        }
    }

    impl Drop for StagingFixture {
        fn drop(&mut self) {
            let _ = fs::remove_file(self.destination());
            let _ = fs::remove_file(staging_file(&self.destination(), self.generation).unwrap());
            let _ = fs::remove_file(staging_file(&self.destination(), self.replacement).unwrap());
            let _ = fs::remove_file(self.legacy());
            let _ = fs::remove_dir(&self.directory);
        }
    }

    #[test]
    fn interrupted_synced_stage_is_recoverable_without_deleting_other_generations() {
        let fixture = StagingFixture::new();
        let destination = fixture.destination();
        fs::write(&destination, b"previous cache generation").unwrap();
        require_staging_available(&fixture.backend(), fixture.generation).unwrap();
        let result = publish_file(
            &destination,
            fixture.generation,
            b"synthetic encrypted stage",
            |stage, _| {
                assert_eq!(fs::read(stage).unwrap(), b"synthetic encrypted stage");
                eyre::bail!("injected interruption after sync before rename")
            },
        );
        let _ = result.unwrap_err();
        let stage = staging_file(&destination, fixture.generation).unwrap();
        assert!(stage.is_file());
        assert_eq!(
            fs::read(&destination).unwrap(),
            b"previous cache generation"
        );
        let replacement = staging_file(&destination, fixture.replacement).unwrap();
        fs::write(&replacement, b"another owned generation").unwrap();
        fs::write(fixture.legacy(), b"old random stage with unknown ownership").unwrap();

        remove_staging(&fixture.backend(), fixture.generation).unwrap();
        remove_staging(&fixture.backend(), fixture.generation).unwrap();
        assert!(!stage.exists());
        assert_eq!(fs::read(replacement).unwrap(), b"another owned generation");
        assert_eq!(
            fs::read(fixture.legacy()).unwrap(),
            b"old random stage with unknown ownership"
        );
        assert_eq!(fs::read(destination).unwrap(), b"previous cache generation");
    }

    #[test]
    fn preexisting_staging_collision_is_preserved() {
        let fixture = StagingFixture::new();
        let stage = staging_file(&fixture.destination(), fixture.generation).unwrap();
        fs::write(&stage, b"preexisting collision").unwrap();
        let _ = require_staging_available(&fixture.backend(), fixture.generation).unwrap_err();
        let _ = publish_file(
            &fixture.destination(),
            fixture.generation,
            b"replacement",
            |_, _| panic!("colliding stage must not reach publication"),
        )
        .unwrap_err();
        assert_eq!(fs::read(stage).unwrap(), b"preexisting collision");
    }

    #[test]
    fn successful_replace_leaves_no_staging_file() {
        let fixture = StagingFixture::new();
        fs::write(fixture.destination(), b"old cache").unwrap();
        publish_file(
            &fixture.destination(),
            fixture.generation,
            b"new encrypted cache",
            replace_file,
        )
        .unwrap();
        assert_eq!(
            fs::read(fixture.destination()).unwrap(),
            b"new encrypted cache"
        );
        assert!(
            !staging_file(&fixture.destination(), fixture.generation)
                .unwrap()
                .exists()
        );
        remove_staging(&fixture.backend(), fixture.generation).unwrap();
    }
}
