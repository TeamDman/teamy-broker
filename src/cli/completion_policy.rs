use teamy_cancellation::CancellationToken;

/// How process presentation handles cancellation racing a successful operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionPolicy {
    CancellationWins,
    ReportConfirmedMutation,
}

impl CompletionPolicy {
    pub(crate) fn check(self, cancellation: &CancellationToken) -> eyre::Result<()> {
        match self {
            Self::CancellationWins => cancellation.bail_if_cancelled(),
            Self::ReportConfirmedMutation => Ok(()),
        }
    }
}
