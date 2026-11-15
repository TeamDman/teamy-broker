use super::descriptor::Descriptor;
use super::lease::Lease;
use crate::platform::cache_reader;
use crate::platform::identity;
use crate::platform::native_storage;
use crate::platform::scheduler;
use crate::protocol::Backend;
use crate::protocol::wire;
use fs2::FileExt;
use sha2::Digest;
use sha2::Sha256;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::path::PathBuf;
use uuid::Uuid;
use windows_sys::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH;
use windows_sys::Win32::Storage::FileSystem::MoveFileExW;
use zeroize::Zeroizing;
fn root() -> eyre::Result<PathBuf> {
    let root = identity::lease_root()?;
    fs::create_dir_all(&root)?;
    Ok(root)
}
fn lock(backend: &Backend) -> eyre::Result<File> {
    let name = match backend {
        Backend::WellKnownFile(path) => {
            let path = Path::new(path);
            let parent = path
                .parent()
                .ok_or_else(|| eyre::eyre!("invalid cache parent"))?
                .canonicalize()?;
            format!(
                "file:{}",
                parent
                    .join(
                        path.file_name()
                            .ok_or_else(|| eyre::eyre!("invalid cache filename"))?
                    )
                    .to_string_lossy()
            )
        }
        Backend::WindowsCredentialManager(target) => format!("credman:{target}"),
        Backend::Memory => "memory".into(),
    };
    let key = format!("{:x}", Sha256::digest(name.to_ascii_lowercase().as_bytes()));
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(root()?.join(format!("{key}.lock")))?;
    file.lock_exclusive()?;
    Ok(file)
}
fn generation(bytes: &[u8]) -> eyre::Result<uuid::Uuid> {
    eyre::ensure!(
        bytes.len() >= 32 && &bytes[..8] == b"TEAMYB01",
        "cache destination collision"
    );
    Ok(uuid::Uuid::from_slice(&bytes[8..24])?)
}

fn remove_descriptor(file: &Path) -> eyre::Result<()> {
    match fs::remove_file(file) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn write_descriptor(file: &Path, descriptor: &Descriptor) -> eyre::Result<()> {
    let encoded = facet_json::to_string(descriptor)?;
    publish_descriptor(file, |metadata| {
        metadata.write_all(encoded.as_bytes())?;
        metadata.sync_all()?;
        Ok(())
    })
}

fn publish_descriptor(
    file: &Path,
    write_and_sync: impl FnOnce(&mut File) -> eyre::Result<()>,
) -> eyre::Result<()> {
    // A process interrupted before rename leaves only unpublished metadata, which reconciliation ignores.
    let staging = file.with_extension("pending");
    let mut metadata = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&staging)?;
    let written = write_and_sync(&mut metadata);
    drop(metadata);
    let result = written.and_then(|()| {
        let source: Vec<u16> = staging.as_os_str().encode_wide().chain(Some(0)).collect();
        let destination: Vec<u16> = file.as_os_str().encode_wide().chain(Some(0)).collect();
        eyre::ensure!(
            // SAFETY: both paths are live, NUL-terminated buffers; no replace flag preserves an existing descriptor.
            unsafe {
                MoveFileExW(
                    source.as_ptr(),
                    destination.as_ptr(),
                    MOVEFILE_WRITE_THROUGH,
                )
            } != 0,
            "cleanup metadata publication failed"
        );
        Ok(())
    });
    if result.is_err() {
        remove_descriptor(&staging)?;
    }
    result
}

fn cleanup_locked(
    file: &Path,
    descriptor: &Descriptor,
    read_cache: impl FnOnce(&Backend) -> eyre::Result<Option<Zeroizing<Vec<u8>>>>,
    delete_cache: impl FnOnce(&Backend) -> eyre::Result<()>,
    remove_staging: impl FnOnce(&Backend, Uuid) -> eyre::Result<()>,
    remove_task: impl FnOnce(&str) -> eyre::Result<()>,
) -> eyre::Result<()> {
    let id = Uuid::parse_str(&descriptor.id)?;
    remove_staging(&descriptor.backend, id)?;
    if let Some(bytes) = read_cache(&descriptor.backend)?
        && generation(&bytes)? == id
    {
        delete_cache(&descriptor.backend)?;
    }
    remove_task(&descriptor.id)?;
    remove_descriptor(file)
}

fn rollback_unpublished(
    file: &Path,
    descriptor: &Descriptor,
    remove_staging: impl FnOnce(&Backend, Uuid) -> eyre::Result<()>,
    remove_task: impl FnOnce(&str) -> eyre::Result<()>,
) -> eyre::Result<()> {
    remove_staging(&descriptor.backend, Uuid::parse_str(&descriptor.id)?)?;
    remove_task(&descriptor.id)?;
    remove_descriptor(file)
}
/// # Errors
///
/// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
pub fn publish(lease: &Lease) -> eyre::Result<()> {
    if lease.request.backend == Backend::Memory {
        return Ok(());
    }
    let _lock = lock(&lease.request.backend)?;
    let id = Uuid::parse_str(&lease.id)?;
    native_storage::require_staging_available(&lease.request.backend, id)?;
    if let Some(bytes) = cache_reader::read(&lease.request.backend)? {
        generation(&bytes)?;
    }
    let descriptor = Descriptor {
        id: lease.id.clone(),
        backend: lease.request.backend.clone(),
        expires: lease.expires,
    };
    let file = root()?.join(format!("{}.json", lease.id));
    // Durable metadata must precede registration. Secret publication must follow verified registration.
    write_descriptor(&file, &descriptor)?;
    if let Err(e) = scheduler::register(&lease.id, lease.expires) {
        let _ = rollback_unpublished(
            &file,
            &descriptor,
            native_storage::remove_staging,
            scheduler::remove,
        );
        return Err(e);
    }
    let mut bytes = Zeroizing::new(Vec::new());
    bytes.extend_from_slice(b"TEAMYB01");
    bytes.extend_from_slice(id.as_bytes());
    bytes.extend_from_slice(&lease.expires.to_be_bytes());
    bytes.extend_from_slice(&lease.secret);
    if let Err(e) = native_storage::write(&lease.request.backend, &bytes, id) {
        let _ = rollback_unpublished(
            &file,
            &descriptor,
            native_storage::remove_staging,
            scheduler::remove,
        );
        return Err(e);
    }
    Ok(())
}
/// # Errors
///
/// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
pub fn cleanup(id: &str, force: bool) -> eyre::Result<()> {
    let id = uuid::Uuid::parse_str(id)?.to_string();
    let file = root()?.join(format!("{id}.json"));
    let descriptor: Descriptor = match fs::read_to_string(&file) {
        Ok(text) => facet_json::from_str(&text)
            .map_err(|_redacted_error| eyre::eyre!("invalid cleanup metadata"))?,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            return scheduler::remove(&id);
        }
        Err(_) => eyre::bail!("cleanup metadata unavailable"),
    };
    eyre::ensure!(descriptor.id == id, "cleanup generation mismatch");
    if !force {
        eyre::ensure!(wire::unix_ms()? >= descriptor.expires, "cleanup is not due");
    }
    let _lock = lock(&descriptor.backend)?;
    cleanup_locked(
        &file,
        &descriptor,
        cache_reader::read,
        native_storage::delete,
        native_storage::remove_staging,
        scheduler::remove,
    )
}
/// Restart never resurrects persisted leases: remove all recognized stale generations first.
/// # Errors
///
/// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
pub fn reconcile() -> eyre::Result<()> {
    let path = identity::lease_root()?;
    if !path.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(path)? {
        let p = entry?.path();
        if p.extension().is_some_and(|e| e == "json") {
            let id = p
                .file_stem()
                .and_then(|s| s.to_str())
                .ok_or_else(|| eyre::eyre!("invalid cleanup name"))?;
            cleanup(id, true)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::env;

    struct TemporaryDescriptor(PathBuf);

    impl TemporaryDescriptor {
        fn new() -> Self {
            let directory =
                env::temp_dir().join(format!("teamy-broker-descriptor-{}", Uuid::new_v4()));
            fs::create_dir(&directory).unwrap();
            Self(directory)
        }

        fn file(&self) -> PathBuf {
            self.0.join("lease.json")
        }
    }

    impl Drop for TemporaryDescriptor {
        fn drop(&mut self) {
            let _ = fs::remove_file(self.file());
            let _ = fs::remove_file(self.file().with_extension("pending"));
            let _ = fs::remove_dir(&self.0);
        }
    }

    fn descriptor() -> Descriptor {
        Descriptor {
            id: Uuid::new_v4().to_string(),
            backend: Backend::WindowsCredentialManager("teamy-broker/synthetic-descriptor".into()),
            expires: 42,
        }
    }

    fn envelope(id: &str) -> Zeroizing<Vec<u8>> {
        let mut bytes = Zeroizing::new(b"TEAMYB01".to_vec());
        bytes.extend_from_slice(Uuid::parse_str(id).unwrap().as_bytes());
        bytes.extend_from_slice(&42_u64.to_be_bytes());
        bytes
    }

    #[test]
    fn partial_descriptor_write_never_publishes_reconciliation_input() {
        let temporary = TemporaryDescriptor::new();
        let file = temporary.file();
        let failure = publish_descriptor(&file, |metadata| {
            metadata.write_all(b"{\"id\":")?;
            metadata.sync_all()?;
            eyre::bail!("injected descriptor write failure")
        });
        let _ = failure.unwrap_err();
        assert!(!file.exists(), "partial metadata became a live descriptor");
        assert!(!file.with_extension("pending").exists());

        let expected = descriptor();
        write_descriptor(&file, &expected).unwrap();
        let published: Descriptor =
            facet_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(published.id, expected.id);
        assert_eq!(published.backend, expected.backend);
        assert_eq!(published.expires, expected.expires);
        assert!(!file.with_extension("pending").exists());
    }

    #[test]
    fn descriptor_publication_preserves_existing_metadata() {
        let temporary = TemporaryDescriptor::new();
        let file = temporary.file();
        let first = descriptor();
        write_descriptor(&file, &first).unwrap();
        let before = fs::read(&file).unwrap();
        let _ = write_descriptor(&file, &descriptor()).unwrap_err();
        assert_eq!(fs::read(&file).unwrap(), before);
        assert!(!file.with_extension("pending").exists());
    }

    #[test]
    fn repeated_cleanup_preserves_replacement_and_accepts_removed_metadata() {
        let temporary = TemporaryDescriptor::new();
        let file = temporary.file();
        let first = descriptor();
        write_descriptor(&file, &first).unwrap();
        let removals = Cell::new(0);
        cleanup_locked(
            &file,
            &first,
            |_| Ok(Some(envelope(&first.id))),
            |_| {
                removals.set(removals.get() + 1);
                Ok(())
            },
            |_, _| Ok(()),
            |_| Ok(()),
        )
        .unwrap();
        assert_eq!(removals.get(), 1);
        assert!(!file.exists());

        // A second cleanup already read the same descriptor before the first held the destination lock.
        let replacement = descriptor();
        cleanup_locked(
            &file,
            &first,
            |_| Ok(Some(envelope(&replacement.id))),
            |_| panic!("older cleanup removed a replacement generation"),
            |_, _| Ok(()),
            |_| Ok(()),
        )
        .unwrap();
    }

    #[test]
    fn failed_task_removal_retains_descriptor_for_retry() {
        let temporary = TemporaryDescriptor::new();
        let file = temporary.file();
        let record = descriptor();
        write_descriptor(&file, &record).unwrap();
        let result = cleanup_locked(
            &file,
            &record,
            |_| Ok(None),
            |_| panic!("absent cache must not be deleted"),
            |_, _| Ok(()),
            |_| eyre::bail!("injected scheduler removal failure"),
        );
        let _ = result.unwrap_err();
        assert!(file.is_file(), "retry metadata was lost");
    }

    #[test]
    fn failed_cache_removal_preserves_task_and_descriptor_for_retry() {
        let temporary = TemporaryDescriptor::new();
        let file = temporary.file();
        let record = descriptor();
        write_descriptor(&file, &record).unwrap();
        let result = cleanup_locked(
            &file,
            &record,
            |_| Ok(Some(envelope(&record.id))),
            |_| eyre::bail!("injected cache removal failure"),
            |_, _| Ok(()),
            |_| panic!("cleanup task was removed before its cache"),
        );
        let _ = result.unwrap_err();
        assert!(file.is_file(), "retry metadata was lost");
    }

    #[test]
    fn failed_staging_removal_preserves_task_and_descriptor_for_retry() {
        let temporary = TemporaryDescriptor::new();
        let file = temporary.file();
        let record = descriptor();
        write_descriptor(&file, &record).unwrap();
        let result = cleanup_locked(
            &file,
            &record,
            |_| panic!("failed staging cleanup must retain all later recovery work"),
            |_| panic!("cache must not be removed after staging failure"),
            |_, _| eyre::bail!("injected staging removal failure"),
            |_| panic!("cleanup task was removed while staging remains"),
        );
        let _ = result.unwrap_err();
        assert!(file.is_file());
    }

    #[test]
    fn unpublished_rollback_keeps_recovery_metadata_until_staging_removal_succeeds() {
        let temporary = TemporaryDescriptor::new();
        let file = temporary.file();
        let record = descriptor();
        write_descriptor(&file, &record).unwrap();
        let result = rollback_unpublished(
            &file,
            &record,
            |_, _| eyre::bail!("injected staging removal failure"),
            |_| panic!("registration must remain retryable while staging remains"),
        );
        let _ = result.unwrap_err();
        assert!(file.is_file());
        rollback_unpublished(&file, &record, |_, _| Ok(()), |_| Ok(())).unwrap();
        assert!(!file.exists());
    }

    #[test]
    fn old_descriptor_cleans_its_partial_stage_and_preserves_a_replacement_cache() {
        let temporary = TemporaryDescriptor::new();
        let file = temporary.file();
        let destination = temporary.0.join("cache");
        let record = Descriptor {
            backend: Backend::WellKnownFile(destination.to_str().unwrap().to_owned()),
            ..descriptor()
        };
        let replacement = envelope(&Uuid::new_v4().to_string());
        fs::write(&destination, &replacement).unwrap();
        let stage = temporary
            .0
            .join(format!(".teamy-broker-stage-{}.tmp", record.id));
        fs::write(&stage, b"incomplete encrypted bytes").unwrap();
        write_descriptor(&file, &record).unwrap();
        cleanup_locked(
            &file,
            &record,
            |_| Ok(Some(Zeroizing::new(fs::read(&destination)?))),
            |_| panic!("old cleanup must preserve replacement cache generation"),
            native_storage::remove_staging,
            |_| Ok(()),
        )
        .unwrap();
        assert!(!stage.exists());
        assert!(!file.exists());
        assert_eq!(fs::read(&destination).unwrap(), *replacement);
        fs::remove_file(destination).unwrap();
    }
}
