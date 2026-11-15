use super::lease_id::LeaseId;
use super::pairing_attempt_id::PairingAttemptId;
use super::phone_authorization_request::PhoneAuthorizationRequest;
use super::purge_scope::PurgeScope;
use crate::broker::phone::config::phone_config_request::PhoneConfigRequest;
use crate::protocol::pairing_comparison::PairingCode;
use std::path::PathBuf;
#[derive(Debug)]
pub enum AppRequest {
    PhoneConfig(PhoneConfigRequest),
    Serve {
        op_executable: PathBuf,
    },
    Status,
    StopWhenIdle,
    StopForce,
    Purge(PurgeScope),
    Cleanup(LeaseId),
    PreviewProposal,
    ApprovalPreview,
    PhonePair,
    PhoneRevoke,
    PhoneList,
    PhoneStatus,
    PhoneAuthorize(PhoneAuthorizationRequest),
    PairingInspect(PairingAttemptId),
    PairingCancel(PairingAttemptId),
    PairingConfirm {
        attempt_id: PairingAttemptId,
        code: PairingCode,
    },
}
