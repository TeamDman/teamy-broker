use super::descriptor::Descriptor;
use super::lease::Lease;
use crate::protocol::Backend;
use fs2::FileExt;
use sha2::Digest;
use sha2::Sha256;
use std::io::Write;
use zeroize::Zeroizing;
fn root() -> eyre::Result<std::path::PathBuf> {
    let root = crate::platform::identity::lease_root()?;
    std::fs::create_dir_all(&root)?;
    Ok(root)
}
fn lock(backend: &Backend) -> eyre::Result<std::fs::File> {
    let name = match backend {
        Backend::WellKnownFile(path) => {
            let path = std::path::Path::new(path);
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
    let file = std::fs::OpenOptions::new()
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
/// # Errors
///
/// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
pub fn publish(lease: &Lease) -> eyre::Result<()> {
    if lease.request.backend == Backend::Memory {
        return Ok(());
    }
    let _lock = lock(&lease.request.backend)?;
    if let Some(bytes) = crate::platform::cache_reader::read(&lease.request.backend)? {
        generation(&bytes)?;
    }
    let descriptor = Descriptor {
        id: lease.id.clone(),
        backend: lease.request.backend.clone(),
        expires: lease.expires,
    };
    let file = root()?.join(format!("{}.json", lease.id));
    // Durable metadata must precede registration. Secret publication must follow verified registration.
    let mut metadata = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&file)?;
    metadata.write_all(facet_json::to_string(&descriptor)?.as_bytes())?;
    metadata.sync_all()?;
    drop(metadata);
    if let Err(e) = crate::platform::scheduler::register(&lease.id, lease.expires) {
        if crate::platform::scheduler::remove(&lease.id).is_ok() {
            let _ = std::fs::remove_file(file);
        }
        return Err(e);
    }
    let mut bytes = Zeroizing::new(Vec::new());
    bytes.extend_from_slice(b"TEAMYB01");
    bytes.extend_from_slice(uuid::Uuid::parse_str(&lease.id)?.as_bytes());
    bytes.extend_from_slice(&lease.expires.to_be_bytes());
    bytes.extend_from_slice(&lease.secret);
    if let Err(e) = crate::platform::native_storage::write(&lease.request.backend, &bytes) {
        if crate::platform::scheduler::remove(&lease.id).is_ok() {
            let _ = std::fs::remove_file(file);
        }
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
    let descriptor: Descriptor = match std::fs::read_to_string(&file) {
        Ok(text) => facet_json::from_str(&text)
            .map_err(|_redacted_error| eyre::eyre!("invalid cleanup metadata"))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return crate::platform::scheduler::remove(&id);
        }
        Err(_) => eyre::bail!("cleanup metadata unavailable"),
    };
    eyre::ensure!(descriptor.id == id, "cleanup generation mismatch");
    if !force {
        eyre::ensure!(
            crate::protocol::wire::unix_ms()? >= descriptor.expires,
            "cleanup is not due"
        );
    }
    let _lock = lock(&descriptor.backend)?;
    if let Some(bytes) = crate::platform::cache_reader::read(&descriptor.backend)?
        && generation(&bytes)? == uuid::Uuid::parse_str(&id)?
    {
        crate::platform::native_storage::delete(&descriptor.backend)?;
    }
    crate::platform::scheduler::remove(&id)?;
    std::fs::remove_file(file)?;
    Ok(())
}
/// Restart never resurrects persisted leases: remove all recognized stale generations first.
/// # Errors
///
/// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
pub fn reconcile() -> eyre::Result<()> {
    let path = crate::platform::identity::lease_root()?;
    if !path.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(path)? {
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
