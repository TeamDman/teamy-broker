use std::sync::Arc;
#[derive(Debug)]
pub struct CredentialResolutionGuard(pub Arc<super::shared_preview::SharedPreview>);
impl Drop for CredentialResolutionGuard {
    fn drop(&mut self) {
        if self.0.outcome().is_none() {
            self.0.invalidate();
        }
        self.0.clear_value();
    }
}
