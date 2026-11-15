use std::fs;
use std::path::PathBuf;
use teamy_broker::broker::persistence;
use teamy_broker::platform::identity;
use teamy_broker::platform::native_storage;
use teamy_broker::protocol::Backend;
use uuid::Uuid;
#[derive(Debug)]
pub struct NativeGuard {
    pub ids: Vec<String>,
    pub backend: Backend,
    pub directory: Option<PathBuf>,
}

impl NativeGuard {
    /// Check production cleanup before disarming the panic fallback.
    /// # Errors
    /// Reports remaining owned metadata or failure to remove the empty fixture directory.
    pub fn finish(mut self) -> eyre::Result<()> {
        let root = identity::lease_root()?;
        for id in &self.ids {
            let id = Uuid::parse_str(id)?;
            for extension in ["json", "pending"] {
                eyre::ensure!(
                    !root.join(format!("{id}.{extension}")).try_exists()?,
                    "fixture cleanup metadata remains"
                );
            }
        }
        if let Some(directory) = &self.directory {
            // An unexpected stage or other file must fail the test, not be swept away.
            fs::remove_dir(directory)?;
            eyre::ensure!(!directory.try_exists()?, "fixture directory remains");
        }
        self.ids.clear();
        self.backend = Backend::Memory;
        self.directory = None;
        Ok(())
    }
}

impl Drop for NativeGuard {
    fn drop(&mut self) {
        for id in &self.ids {
            let _ = persistence::cleanup(id, true);
        }
        let _ = native_storage::delete(&self.backend);
        if let Some(directory) = &self.directory {
            let _ = fs::remove_dir(directory);
        }
    }
}
