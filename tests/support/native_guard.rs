use teamy_broker::protocol::Backend;
#[derive(Debug)]
pub struct NativeGuard {
    pub ids: Vec<String>,
    pub backend: Backend,
    pub directory: Option<std::path::PathBuf>,
}
impl Drop for NativeGuard {
    fn drop(&mut self) {
        for id in &self.ids {
            let _ = teamy_broker::broker::persistence::cleanup(id, true);
        }
        let _ = teamy_broker::platform::native_storage::delete(&self.backend);
        if let Some(directory) = &self.directory {
            let _ = std::fs::remove_dir(directory);
        }
    }
}
