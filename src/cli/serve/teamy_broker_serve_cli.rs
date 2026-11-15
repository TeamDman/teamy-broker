use crate::broker::app_request::AppRequest;
use arbitrary::Arbitrary;
use facet::Facet;
use figue::{self as args};
use std::path::PathBuf;
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct ServeArgs {
    /// Explicit path to 1Password CLI. Default resolves op.exe through PATH.
    #[facet(args::named)]
    pub op_executable: Option<String>,
}

impl ServeArgs {
    /// # Errors
    /// Rejects an empty executable path before starting the daemon.
    pub fn into_request(self) -> eyre::Result<AppRequest> {
        let op_executable = PathBuf::from(self.op_executable.unwrap_or_else(|| "op.exe".into()));
        eyre::ensure!(
            !op_executable.as_os_str().is_empty(),
            "provider executable required"
        );
        Ok(AppRequest::Serve { op_executable })
    }
}
