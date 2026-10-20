# Command completion budgets

Every successful subcommand returns `CliOutput` with an explicit `elapsed_duration_warn_threshold: Option<Duration>`. Constructors require the budget: `CliOutput::facet(value, Some(Duration::from_secs(1)))` or `CliOutput::none(None)`. There is no default or CLI-struct attribute. Parent dispatch forwards the receipt without replacing its budget. The field is execution metadata and is never added to JSON, CSV or text result data.

The top level captures `Instant::now()` immediately before invocation, renders output, then compares elapsed time to the returned budget. It warns only for strictly greater elapsed time. Fields are `elapsed_duration_ms` and `elapsed_duration_warn_threshold_ms`. This completion-time warning cannot detect a hung command or warn before a result returns. Invocation failures have no returned budget and keep their existing safe diagnostics. A returned output whose rendering fails still has its budget checked; the original rendering error remains the result.

Choose budgets from expected work, not from observed elapsed time. Path inspection receives one second; detached folder opening receives three seconds. Template init receives three seconds plus 20 ms per inventory file. `None` is an explicit exemption for indefinitely running serve/watch, unbounded archival or user-interactive approval operations; document why at the return site. Short planning variants of interactive commands should still return a finite budget. Separate phases may emit useful counts, bytes, rates or ETA directly through tracing; no mandatory progress schema is involved.

Migration checklist:

1. Add the output receipt and elapsed comparison helper; retain existing result serialization and redaction.
2. Require a budget argument on every output constructor; remove default constructors and fixed command/argument attributes.
3. Give every leaf operation an explicit budget expression based on its work/options; document every exemption. Forward leaf receipts through all dispatch groups.
4. Capture the top-level Instant before dispatch and check after output, including output errors. Preserve direct DEBUG dispatch/completion and command-span timing.
5. Test below/equal/above and explicit exemption without sleeps; verify serialized result schemas remain unchanged and diagnostics remain redacted.
6. Run formatting, strict all-target Clippy, and full tests before installation. Propagate to other consumers only after their ownership is coordinated.
`teamy-broker` retains its existing ten-second completion budget for status, purge, stop and cleanup. These are bounded control or maintenance operations. Foreground serve explicitly returns no budget because it runs until stopped and includes user-interactive approvals.
