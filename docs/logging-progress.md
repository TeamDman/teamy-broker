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

Producer code must choose safe fields. Never log credentials, item metadata, raw CLI arguments or arbitrary panic/error payloads. Use `skip_all` when instrumenting sensitive inputs. Subprocess regressions verify that synthetic private markers cannot appear in startup failures.

Emit direct tracing events at meaningful call sites. Choose DEBUG for routine dispatch and completion, INFO for useful operation updates, and WARN/ERROR for reviewed failures. Add operation-specific structured fields, such as completed_items, bytes_written, elapsed_ms, or phase, when useful. There is no mandatory progress schema, universal task/run/state vocabulary, or custom caller-location wrapper. Text and NDJSON retain tracing's actual event file/line. Existing command spans still provide timing receipts, and existing expected-duration checks remain warning-only. Logs do not replace command results or typed service status.

Broker IPC does not transmit diagnostic streams or request log filters. Daemon diagnostics remain in its explicitly configured local sinks; status exposes redacted counters. The safe-message callback classifies errors into reviewed static text without exposing raw chains.


## Completion budgets

The CLI output adapter assigns an explicit optional completion budget. Status, purge, stop, cleanup and local phone configuration use ten seconds. Foreground serving and interactive phone/approval operations are exempt. These are warning thresholds, separate from transport and approval deadlines.

The top level measures invocation and output together, then warns only when elapsed time exceeds the returned budget. Diagnostics contain `elapsed_duration_ms` and `elapsed_duration_warn_threshold_ms`; budgets never enter result JSON, CSV or text. Rendering failures still receive the budget check, while retaining the original rendering error. A completion warning cannot detect a hung command or predict remaining work.
