pub mod cleanup;
pub mod command;
pub mod facet_shape;
pub mod global_args;
pub mod log_format;
pub mod output;
pub mod phone_authorize;
pub mod phone_list;
pub mod phone_pair;
pub mod phone_pair_cancel;
pub mod phone_pair_confirm;
pub mod phone_pair_inspect;
pub mod phone_revoke;
pub mod phone_status;
pub mod preview;
pub mod purge;
pub mod serve;
pub mod status;
pub mod stop;
use arbitrary::Arbitrary;
pub use command::Command;
use facet::Facet;
use figue::FigueBuiltins;
use figue::{self as args};
use global_args::GlobalArgs;
use output::CliOutput;
use teamy_cancellation::CancellationToken;
/// Per-user Windows credential broker. Secrets are available only through the SDK's explicit acquisition API.
#[derive(Facet, Arbitrary, Debug)]
pub struct Cli {
    #[facet(flatten)]
    pub global_args: GlobalArgs,
    #[facet(flatten)]
    #[arbitrary(default)]
    pub builtins: FigueBuiltins,
    #[facet(args::subcommand)]
    pub command: Command,
}
impl PartialEq for Cli {
    fn eq(&self, other: &Self) -> bool {
        self.global_args == other.global_args && self.command == other.command
    }
}
impl Cli {
    /// # Errors
    ///
    /// Returns an error on invalid configuration, expired leases, denial, or failed I/O.
    pub fn invoke(self, cancellation: CancellationToken) -> eyre::Result<CliOutput> {
        tracing::info!(phase = "request", "Command startup");
        let request = self.command.into_request()?;
        tracing::info!(phase = "runtime-create", "Command startup");
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        tracing::info!(phase = "runtime-ready", "Command startup");
        runtime.block_on(crate::broker::dispatch::invoke(request, cancellation))
    }
}
