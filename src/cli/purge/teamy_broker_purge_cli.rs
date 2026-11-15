use crate::broker::app_request::AppRequest;
use crate::broker::purge_scope::PurgeScope;
use arbitrary::Arbitrary;
use facet::Facet;
use figue::{self as args};
#[derive(Facet, Arbitrary, Debug, PartialEq)]
#[facet(rename_all = "kebab-case")]
pub struct PurgeArgs {
    /// Purge all leases. Mutually exclusive with --lease-id.
    #[facet(args::named, default)]
    pub all: bool,
    /// Opaque lease ID returned by SDK. Mutually exclusive with --all.
    #[facet(args::named)]
    pub lease_id: Option<String>,
}

impl PurgeArgs {
    /// # Errors
    /// Requires exactly one scope and a valid lease UUID when selecting one lease.
    pub fn into_request(self) -> eyre::Result<AppRequest> {
        let scope = match (self.all, self.lease_id) {
            (true, None) => PurgeScope::All,
            (false, Some(id)) => PurgeScope::Lease(id.try_into()?),
            _ => eyre::bail!("select exactly one of --all or --lease-id"),
        };
        Ok(AppRequest::Purge(scope))
    }
}
