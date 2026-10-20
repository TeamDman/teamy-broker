use arbitrary::Arbitrary;
use facet::Facet;
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum Command {
    /// Run the foreground per-user daemon; no service installation or elevation.
    Serve(super::serve::ServeArgs),
    /// Arm one five-minute phone no-vault enrollment attempt; compare both screens or use exact-ID CLI confirmation.
    PhonePair(super::phone_pair::PhonePairArgs),
    /// Scoped ephemeral pairing comparison control; never expands credential permissions.
    PhonePairCancel(super::phone_pair_cancel::PhonePairCancelArgs),
    /// Scoped ephemeral pairing comparison control; never expands credential permissions.
    PhonePairConfirm(super::phone_pair_confirm::PhonePairConfirmArgs),
    /// Scoped ephemeral pairing comparison control; never expands credential permissions.
    PhonePairInspect(super::phone_pair_inspect::PhonePairInspectArgs),
    /// Revoke the enrolled phone identity, close its connection/listener, and cancel any pairing window.
    PhoneRevoke(super::phone_revoke::PhoneRevokeArgs),
    /// List the one exact no-vault phone relationship without keys or certificates.
    PhoneList(super::phone_list::PhoneListArgs),
    /// Show redacted phone relationship, pairing window, listener and connection status.
    PhoneStatus(super::phone_status::PhoneStatusArgs),
    /// Authorize one exact phone credential action; supply requires explicit --supply.
    PhoneAuthorize(super::phone_authorize::PhoneAuthorizeArgs),
    /// Show redacted daemon and lease counts. Does not autostart.
    Status(super::status::StatusArgs),
    /// Propose a harmless resident attention check; opening it requires --show-test-dialog.
    Preview(super::preview::PreviewArgs),
    /// Purge one opaque lease ID or all broker-owned leases.
    Purge(super::purge::PurgeArgs),
    /// Stop only when no work or cached leases remain; --force authorizes maintenance interruption.
    Stop(super::stop::StopArgs),
    /// Internal scheduled cleanup: opaque lease ID only.
    Cleanup(super::cleanup::CleanupArgs),
}
impl Command {
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Serve(_) => "serve",
            Self::PhonePair(_) => "phone-pair",
            Self::PhonePairCancel(_) => "phone-pair-cancel",
            Self::PhonePairConfirm(_) => "phone-pair-confirm",
            Self::PhonePairInspect(_) => "phone-pair-inspect",
            Self::PhoneRevoke(_) => "phone-revoke",
            Self::PhoneList(_) => "phone-list",
            Self::PhoneStatus(_) => "phone-status",
            Self::PhoneAuthorize(_) => "phone-authorize",
            Self::Status(_) => "status",
            Self::Preview(_) => "preview",
            Self::Purge(_) => "purge",
            Self::Stop(_) => "stop",
            Self::Cleanup(_) => "cleanup",
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
            Self::PhonePair(_) => AppRequest::Control(Request::control("phone-pair", None)),
            Self::PhonePairCancel(args) => {
                uuid::Uuid::parse_str(&args.attempt_id)
                    .map_err(|_redacted_error| eyre::eyre!("invalid attempt ID"))?;
                AppRequest::Control(Request::control("phone-pair-cancel", Some(args.attempt_id)))
            }

            Self::PhonePairConfirm(args) => {
                uuid::Uuid::parse_str(&args.attempt_id)
                    .map_err(|_redacted_error| eyre::eyre!("invalid attempt ID"))?;
                eyre::ensure!(args.comparison_stdin, "--comparison-stdin required");
                let code = super::phone_pair_confirm::read_code(
                    std::io::stdin(),
                    std::time::Duration::from_secs(5),
                )?;
                AppRequest::PairingConfirm {
                    attempt_id: args.attempt_id,
                    code,
                }
            }

            Self::PhonePairInspect(args) => {
                uuid::Uuid::parse_str(&args.attempt_id)
                    .map_err(|_redacted_error| eyre::eyre!("invalid attempt ID"))?;
                AppRequest::Control(Request::control(
                    "phone-pair-inspect",
                    Some(args.attempt_id),
                ))
            }

            Self::PhoneRevoke(_) => AppRequest::Control(Request::control("phone-revoke", None)),
            Self::PhoneList(_) => AppRequest::Control(Request::control("phone-list", None)),
            Self::PhoneStatus(_) => AppRequest::Control(Request::control("phone-status", None)),
            Self::PhoneAuthorize(args) => AppRequest::Control(args.request()?),
            Self::Preview(args) => AppRequest::Preview(args.show_test_dialog),
            Self::Stop(args) => AppRequest::Control(Request::control(
                if args.force { "stop-force" } else { "stop" },
                None,
            )),
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
