#[derive(Debug)]
pub enum AppRequest {
    Serve(std::path::PathBuf),
    Control(crate::protocol::Request),
    Cleanup(String),
    Preview(bool),
    PairingConfirm {
        attempt_id: String,
        code: crate::protocol::pairing_comparison::PairingCode,
    },
}
