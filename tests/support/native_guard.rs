use std::fs;
use std::path::PathBuf;
use teamy_broker::broker::persistence;
use teamy_broker::platform::native_storage;
use teamy_broker::protocol::Backend;
#[derive(Debug)]
pub struct NativeGuard {
    pub ids: Vec<String>,
    pub backend: Backend,
    pub directory: Option<PathBuf>,
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
