#[derive(Debug)]
pub enum AppRequest {
    Serve(std::path::PathBuf),
    Control(crate::protocol::Request),
    Cleanup(String),
}
