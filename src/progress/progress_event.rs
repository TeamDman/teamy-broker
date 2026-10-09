use super::ProgressState;

/// Version 1 progress fields. IDs and labels are producer-owned, never raw arguments.
/// `completed` is cumulative within this task; omitted `total` means unknown.
#[derive(Clone, Debug)]
pub struct ProgressEvent<'a> {
    pub run_id: &'a str,
    pub task_id: &'a str,
    pub parent_task_id: Option<&'a str>,
    pub operation: &'a str,
    pub phase: &'a str,
    pub state: ProgressState,
    pub completed: u64,
    pub total: Option<u64>,
    pub unit: &'a str,
}

impl ProgressEvent<'_> {
    /// Emit a numeric, correlated progress event without any GUI dependency.
    ///
    /// # Errors
    /// Rejects empty labels/identifiers or a count exceeding a known total.
    pub fn emit(&self) -> eyre::Result<()> {
        eyre::ensure!(
            !self.run_id.is_empty()
                && !self.task_id.is_empty()
                && !self.operation.is_empty()
                && !self.phase.is_empty()
                && !self.unit.is_empty(),
            "progress identifiers and labels must be nonempty"
        );
        eyre::ensure!(
            self.parent_task_id != Some(self.task_id),
            "task cannot parent itself"
        );
        eyre::ensure!(
            self.parent_task_id != Some(""),
            "parent task identifier must be nonempty"
        );
        eyre::ensure!(
            self.total.is_none_or(|total| self.completed <= total),
            "completed progress exceeds total"
        );
        tracing::info!(target: "teamy::progress",
            event_kind = "progress", schema_version = 1_u64,
            run_id = self.run_id, task_id = self.task_id, parent_task_id = self.parent_task_id,
            operation = self.operation, phase = self.phase, state = self.state.as_str(),
            completed = self.completed, total = self.total, unit = self.unit,
            "Task progress");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_progress_is_rejected_without_emission() {
        let mut event = ProgressEvent {
            run_id: "run",
            task_id: "task",
            parent_task_id: None,
            operation: "scan",
            phase: "scan",
            state: ProgressState::Progress,
            completed: 3,
            total: Some(2),
            unit: "items",
        };
        assert!(event.emit().is_err());
        event.total = None;
        event.emit().unwrap();
        event.parent_task_id = Some("task");
        assert!(event.emit().is_err());
    }
}
