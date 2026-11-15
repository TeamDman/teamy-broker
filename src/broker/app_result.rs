use super::phone::config::phone_config_report::PhoneConfigReport;
use crate::protocol::ApprovalPreviewResult;
use crate::protocol::PhoneStatus;
use crate::protocol::Status;
use crate::protocol::pairing_comparison::PairingConfirmationResult;
use crate::protocol::pairing_comparison::PairingInspection;

/// Completed operation data. Formatting and process presentation belong to the caller.
#[derive(Debug)]
pub enum AppResult {
    Completed,
    Status(Status),
    PhoneStatus(PhoneStatus),
    PhoneConfig(PhoneConfigReport),
    PairingInspection(PairingInspection),
    PairingConfirmation(PairingConfirmationResult),
    ApprovalPreview(ApprovalPreviewResult),
    PreviewProposal,
}
