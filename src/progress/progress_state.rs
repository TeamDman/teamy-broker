/// Terminal states describe the task outcome, independently of GUI presentation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgressState {
    Started,
    Progress,
    Completed,
    Failed,
    Cancelled,
}

impl ProgressState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Progress => "progress",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}
