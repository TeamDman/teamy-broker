use crate::protocol::Request;
use crate::protocol::pairing_comparison::PairingCode;
use std::path::PathBuf;
#[derive(Debug)]
pub enum AppRequest {
    Serve(PathBuf),
    Control(Request),
    Cleanup(String),
    Preview(bool),
    PairingConfirm {
        attempt_id: String,
        code: PairingCode,
    },
}
