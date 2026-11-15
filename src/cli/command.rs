use super::completion_policy::CompletionPolicy;
use super::output::OutputFormat;
use crate::broker::app_request::AppRequest;
use crate::protocol::pairing_comparison::PairingCode;
use arbitrary::Arbitrary;
use facet::Facet;
use std::time::Duration;
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[repr(u8)]
pub enum Command {
    /// Run the foreground per-user daemon; no service installation or elevation.
    Serve(super::serve::ServeArgs),
    /// Save runtime phone settings while the broker is stopped; revoke before changing an enrolled profile.
    PhoneConfigure(super::phone_configure::PhoneConfigureArgs),
    /// Show local profile readiness; private values require --show-values.
    PhoneConfigShow(super::phone_config_show::PhoneConfigShowArgs),
    /// Validate the local profile and Java executable without connecting or changing trust.
    PhoneConfigValidate(super::phone_config_validate::PhoneConfigValidateArgs),
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
    pub const fn completion_policy(&self) -> CompletionPolicy {
        match self {
            Self::PhoneConfigure(_)
            | Self::PhonePair(_)
            | Self::PhoneRevoke(_)
            | Self::PhoneAuthorize(_)
            | Self::PhonePairCancel(_)
            | Self::PhonePairConfirm(_)
            | Self::Stop(_)
            | Self::Purge(_)
            | Self::Cleanup(_) => CompletionPolicy::ReportConfirmedMutation,
            Self::Serve(_)
            | Self::PhoneConfigShow(_)
            | Self::PhoneConfigValidate(_)
            | Self::PhoneList(_)
            | Self::PhoneStatus(_)
            | Self::PhonePairInspect(_)
            | Self::Status(_)
            | Self::Preview(_) => CompletionPolicy::CancellationWins,
        }
    }

    #[must_use]
    pub const fn elapsed_duration_warn_threshold(&self) -> Option<Duration> {
        match self {
            Self::PhoneConfigure(_)
            | Self::PhoneConfigShow(_)
            | Self::PhoneConfigValidate(_)
            | Self::Status(_)
            | Self::Stop(_)
            | Self::Purge(_)
            | Self::Cleanup(_) => Some(Duration::from_secs(10)),
            Self::Preview(args) if !args.show_test_dialog => Some(Duration::from_secs(10)),
            Self::Serve(_)
            | Self::PhonePair(_)
            | Self::PhoneRevoke(_)
            | Self::PhoneAuthorize(_)
            | Self::PhonePairCancel(_)
            | Self::PhonePairConfirm(_)
            | Self::PhoneList(_)
            | Self::PhoneStatus(_)
            | Self::PhonePairInspect(_)
            | Self::Preview(_) => None,
        }
    }

    /// # Errors
    /// Rejects output formats that cannot represent this command's result before execution.
    pub fn validate_output_format(&self, requested: Option<OutputFormat>) -> eyre::Result<()> {
        super::output_policy::validate(self, requested)
    }

    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Serve(_) => "serve",
            Self::PhoneConfigure(_) => "phone-configure",
            Self::PhoneConfigShow(_) => "phone-config-show",
            Self::PhoneConfigValidate(_) => "phone-config-validate",
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
    /// Returns an error on invalid configuration or missing collected comparison input.
    /// This conversion performs no I/O; CLI invocation collects stdin separately.
    pub fn into_request(self) -> eyre::Result<AppRequest> {
        self.into_request_with_comparison(None)
    }

    pub(crate) fn into_request_with_comparison(
        self,
        comparison: Option<PairingCode>,
    ) -> eyre::Result<AppRequest> {
        Ok(match self {
            Self::PhoneConfigure(args) => AppRequest::PhoneConfig(args.into_request()?),
            Self::PhoneConfigShow(args) => AppRequest::PhoneConfig(args.into_request()?),
            Self::PhoneConfigValidate(args) => AppRequest::PhoneConfig(args.into_request()?),
            Self::Serve(args) => args.into_request()?,
            Self::Status(_) => AppRequest::Status,
            Self::PhonePair(_) => AppRequest::PhonePair,
            Self::PhonePairCancel(args) => args.into_request()?,
            Self::PhonePairConfirm(args) => args.into_request(comparison)?,
            Self::PhonePairInspect(args) => args.into_request()?,
            Self::PhoneRevoke(_) => AppRequest::PhoneRevoke,
            Self::PhoneList(_) => AppRequest::PhoneList,
            Self::PhoneStatus(_) => AppRequest::PhoneStatus,
            Self::PhoneAuthorize(args) => AppRequest::PhoneAuthorize(args.into_request()?),
            Self::Preview(args) => args.into_request(),
            Self::Stop(args) => args.into_request(),
            Self::Purge(args) => args.into_request()?,
            Self::Cleanup(args) => args.into_request()?,
        })
    }
}
