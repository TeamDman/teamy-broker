pub mod cleanup;
pub mod command;
pub mod facet_shape;
pub mod global_args;
pub mod log_format;
pub mod output;
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
        let request = self.command.into_request()?;
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
            .block_on(crate::broker::dispatch::invoke(request, cancellation))
    }
}
