# Logging and progress contract

Command results use stdout and the existing `--output-format` contract. Diagnostics use stderr and an optional explicitly named file.

| Setting | Default |
| --- | --- |
| `--log-format text\|ndjson` | text |
| `--log-filter DIRECTIVES` | explicit flag, then `--debug`, then `RUST_LOG`, then info |
| `--log-file FILE` | no file sink |
| `--log-file-format text\|ndjson` | ndjson |
| `--log-file-filter DIRECTIVES` | debug, independent of stderr and environment |

An explicit stderr filter wins over `--debug`; `--log-level` remains its existing alias. Filters accept full EnvFilter directive lists. Regex matching in field directives is disabled. File filters never inherit stderr filters. Both are attached to their own layers, so INFO stderr cannot suppress DEBUG file events. Tracy retains its separate collection policy.

Files append, create missing parent directories, and require a filename rather than a directory. A mutex guard covers each full formatted file record. JSON sinks and file text never emit ANSI. All filters are validated before file effects; subscriber installation failures are errors. Do not use explicit log files as a durable audit guarantee: there is no fsync/rotation policy.

The binary boundary emits safe failure records without raw errors, arguments, paths, or panic payloads. In NDJSON mode these are JSON lines even if startup fails or diagnostics are disabled. Explicit help/version/completions retain Figue's ordinary presentation. Their display is outside the diagnostic stream contract. Library callers of `main()` own error and panic presentation themselves.

This is a producer discipline, not automatic general redaction. Never log credentials, mail bodies, tokens, raw CLI arguments or arbitrary panic/error payloads. Choose explicit safe fields and use `skip_all` on instrumentation of sensitive inputs. The subprocess regression uses a synthetic secret marker and verifies that a startup failure cannot disclose it.

Version 1 progress is emitted at INFO under `teamy::progress` with numeric `schema_version`, `completed`, and known `total`; fields also include `event_kind=progress`, `run_id`, `task_id`, optional `parent_task_id`, `operation`, `phase`, `state`, and `unit`. Counts are cumulative within a task. Absent total means unknown. States are started, progress, completed, failed, and cancelled. IDs/labels are producer-owned, never raw user arguments. The top-level command emits started and a terminal outcome; long operations can use `progress::ProgressEvent` for child tasks and phases. Filters may hide progress; it does not replace result or durable state contracts.

There is no GUI or OpenTelemetry dependency. A downstream application can interpret the fields directly.

V1 broker IPC does not transmit diagnostic streams or request log filters. Daemon diagnostics remain in its explicitly configured local sinks; status exposes redacted counters. The safe-message callback classifies errors into reviewed static text without exposing raw chains.
