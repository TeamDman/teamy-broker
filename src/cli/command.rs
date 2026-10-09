use arbitrary::Arbitrary;
use facet::Facet;
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum Command {
    /// Run the foreground per-user daemon; no service installation or elevation.
    Serve(super::serve::ServeArgs),
    /// Show redacted daemon and lease counts. Does not autostart.
    Status(super::status::StatusArgs),
    /// Purge one opaque lease ID or all broker-owned leases.
    Purge(super::purge::PurgeArgs),
    /// Stop the daemon gracefully and purge its leases.
    Stop(super::stop::StopArgs),
    /// Internal scheduled cleanup: opaque lease ID only.
    Cleanup(super::cleanup::CleanupArgs),
}
impl Command {
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Serve(_) => "serve",
            Self::Status(_) => "status",
            Self::Purge(_) => "purge",
            Self::Stop(_) => "stop",
            Self::Cleanup(_) => "cleanup",
        }
    }
    #[must_use]
    pub fn expected_duration(&self) -> Option<std::time::Duration> {
        if matches!(self, Self::Serve(_)) {
            None
        } else {
            Some(std::time::Duration::from_secs(10))
        }
    }
    /// # Errors
    ///
    /// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
    pub fn into_request(self) -> eyre::Result<crate::broker::app_request::AppRequest> {
        use crate::broker::app_request::AppRequest;
        use crate::protocol::Request;
        Ok(match self {
            Self::Serve(args) => AppRequest::Serve(std::path::PathBuf::from(
                args.op_executable.unwrap_or_else(|| "op.exe".into()),
            )),
            Self::Status(_) => AppRequest::Control(Request::control("status", None)),
            Self::Stop(_) => AppRequest::Control(Request::control("stop", None)),
            Self::Purge(args) => {
                eyre::ensure!(
                    args.all ^ args.lease_id.is_some(),
                    "select exactly one of --all or --lease-id"
                );
                let request = Request::control("purge", args.lease_id);
                request.validate()?;
                AppRequest::Control(request)
            }
            Self::Cleanup(args) => {
                uuid::Uuid::parse_str(&args.lease_id)
                    .map_err(|_redacted_error| eyre::eyre!("invalid lease ID"))?;
                AppRequest::Cleanup(args.lease_id)
            }
        })
    }
}
