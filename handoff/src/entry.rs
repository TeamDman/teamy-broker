use crate::deadline::Deadline;
use crate::received_decision::ReceivedDecision;
use tokio::sync::oneshot;
pub(crate) struct Entry {
    pub binding: String,
    pub deadline: Deadline,
    pub sender: oneshot::Sender<ReceivedDecision>,
}
