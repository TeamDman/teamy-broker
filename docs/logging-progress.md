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

Emit direct tracing events at meaningful call sites. Choose DEBUG for routine dispatch and completion, INFO for useful operation updates, and WARN/ERROR for reviewed failures. Add operation-specific structured fields, such as completed_items, bytes_written, elapsed_ms, or phase, when useful. There is no mandatory progress schema, universal task/run/state vocabulary, or custom caller-location wrapper. Text and NDJSON retain tracing's actual event file/line. Existing command spans still provide timing receipts, and existing expected-duration checks remain warning-only. Logs do not replace command results or typed service status.

There is no GUI or OpenTelemetry dependency. A downstream application can interpret the fields directly.

Broker IPC does not transmit diagnostic streams or request log filters. Daemon diagnostics remain in its explicitly configured local sinks; status exposes redacted counters. The safe-message callback classifies errors into reviewed static text without exposing raw chains.


Command runtime expectations describe a command contract (for example, expected
completion under one second where that contract already exists). They are not
work progress or an ETA. Actual work may emit item/byte counts, remaining work,
rates, or evidence-based item/byte ETAs as operation-specific fields. Consumers
may derive rates and ETAs from suitable measurements. Do not substitute command
completed=0/1, mandatory percentages, or invented estimates for those measurements.

```rust
tracing::debug!(items_processed = processed, bytes_written = written, "Batch written");
```

Use safe fields at the actual operation call site; no helper envelope is required.
