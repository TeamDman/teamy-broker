use std::sync::Arc;
use teamy_broker_handoff::binding::Binding;
use teamy_broker_handoff::queue::Queue;
/// Only the synthetic broadcast dialog uses this state; production approval is untouched.
#[derive(Debug)]
pub(super) struct DesktopPromptState {
    pub queue: Arc<Queue>,
    pub binding: Binding,
    pub work: crate::broker::work_guard::WorkGuard,
}
