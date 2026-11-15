#[cfg(test)]
use crate::broker::shared_preview::SharedPreviewHub;
#[cfg(test)]
use std::sync::Arc;

#[derive(Debug)]
pub(super) enum PreviewHub {
    Resident,
    #[cfg(test)]
    Isolated(Arc<SharedPreviewHub>),
}
