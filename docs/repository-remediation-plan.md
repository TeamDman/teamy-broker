# Broker repository remediation plan

**Plan status:** Active; implementation authorized and underway
**Primary implementation root:** `teamy-broker/`, currently branch `main`
**Last updated:** 2026-11-15
**Intent audit:** Passed 2026-11-15 against the repository instructions and the full remediation request in this conversation
**Current focus:** 1.3 checkpoint, followed by 2.3 runtime configuration and 3.1 production module ownership

The immediate goal is a reviewed, coherent Git checkpoint. The larger goal is a broker whose supported workflows, module ownership, configuration and tests can be understood and maintained. Reviewing the pending diff alone cannot establish that larger outcome.

The user has authorized running this remediation to completion, including routine implementation decisions and coherent commits. Cleanup happens within each reviewed subsystem: trace supported behavior, repair and refactor it, remove confirmed obsolete material, validate and commit. Phase 5 is the final completeness audit, not the first opportunity to prune files. Material product or supported-functionality changes still need user direction. Deployment, publication and live phone trust changes remain separate.

Parallel ownership: root owns runtime configuration and this plan; `broker_topology` owns production phone module extraction; `android_feasibility` owns the bounded portable-profile/JNI trial; `reference_patterns` owns startup/output contracts and repository tooling. The initial import repairs are complete. Heavy builds are coordinated by root, and unpublished trial artifacts stay under ignored `ignore/`.

## How to update this plan

- `[ ]` Not started; `[~]` In progress; `[x]` Complete; `[!]` Blocked.
- Put status in each work-item heading. Update its completion notes at the same time. A phase is complete only when every item is `[x]`.
- Keep one implementation focus. If work is split between people or agents, name the independent owners and file boundaries here.
- Record decisions, commit IDs, commands and their outcomes under the affected task. Historical test counts are not current validation.
- Use `[!]` only with the exact blocker, latest evidence and condition that unblocks it. On a disk-space error, stop affected work and wait for the user; do not prune files or caches.

## User guidance and traceability

These entries preserve the user's direction. U17–U18 update the original planning-only scope and execution order; unresolved product choices remain behind their decision gates.

| ID | Active guidance | Work and completion evidence |
| --- | --- | --- |
| U1 | Focus on broker while auditing changes across several projects; review the entire project, including unchanged code. | 2.1 and 3.3 cover all owned subsystems; 5.3 records remaining limits. |
| U2 | Reach a clean working tree as part of cleanup; reviewing only staged/unstaged changes is insufficient. | 1.1–1.3 establish reviewed checkpoints; 5.3 checks final Git status. |
| U3 | Apply maintained TeamyStyle. | Reference R2, capability audit in 2.1, and refactors in 3.1–3.3. |
| U4 | Audit whether broker used and complies with the general Teamy Rust CLI foundation. | 0.1 establishes claimed lineage; 2.1 compares capabilities rather than copying a snapshot. |
| U5 | Give Cloud Terrastodon's curated approaches weight; that work has informed the CLI template. | R6 and the concrete examples below guide request/result, type and lifecycle work. |
| U6 | Investigate Makepad's Rust Android approach; prefer Rust-owned logic over duplicated Rust/Java logic where feasible. This is a preference to evaluate, not a selected framework. | G3 and 4.1–4.2 require a bounded trial and parity evidence. |
| U7 | Review excess examples, including those perceived as SDK examples. | 2.1 maps the root examples and SDK consumers; 5.1 keeps, relocates or removes each with a reason. |
| U8 | Resolve misleading mobile/dummy naming, especially where supposedly dummy code is used by the implementation. | 3.1 separates live phone capabilities from synthetic support; 5.2 corrects descriptions. |
| U9 | Start from the CLI and identify unused code for removal. | 2.1 follows the CLI and then its SDK, Android, subprocess and test roots; 5.1 requires evidence before deletion. |
| U10 | Clean up documents, tests, decisions and other leftovers from previous LLM iterations. | 1.1 reviews untracked material; 5.1–5.2 retain useful behavior/provenance and remove stale duplication. |
| U11 | Device configuration must be user-configurable at runtime through the CLI/service workflow, rather than baked into builds. | G2 and 2.2–2.3 finish the partial runtime migration and prove configuration changes need no rebuild. |
| U12 | Start with a plan saved in broker's docs folder, using the supplied resumable-plans approach. | Completed by this document and 0.1; the planning-only boundary is superseded by U17. |
| U13 | Use the supplied repositories, style file, skill and existing workspace as references. | R1–R7 retain all seven references in portable form; unrelated workspace projects stay outside the implementation scope. |
| U14 | Resolve voice-transcribed Timmy/Timi/Tammy references as Teamy; Cloud Terrastalon refers to Cloud Terrastodon here. | Names verified against supplied checkouts and used consistently throughout. |
| U15 | Each subcommand has its own directory and a discoverable `*_cli.rs` implementation re-exported by `mod.rs`. | Preserve `AGENTS.md` and existing `teamy_broker_*_cli.rs` naming in 2.3 and 3.2. |
| U16 | Installed tools may require execution outside the sandbox; disk-space errors must wait for the user. | Tooling notes in 0.1 and validation rules below; no automatic cleanup task. |
| U17 | Set the goal and continue through the agreed remediation to completion; make routine implementation decisions and coherent commits autonomously. | Active goal; all work items. Material product decisions and live deployment/trust changes retain their stated boundaries. |
| U18 | Remove confirmed dead code and redundant material within each reviewed subsystem so cleanup is visible throughout execution. | 2.1, 3.1–3.3 and 4.2 include immediate pruning and validation; 5.1–5.2 become final coverage checks. Supersedes deferring broad cleanup until Phase 5. |

## Intent audit evidence

- **Pass 1, extraction:** Reread the supplied AGENTS instructions, seven paths, full voice-transcribed request and follow-up authorizing the goal and revised sequencing. U1–U18 preserve that direction; U17–U18 record the transition from planning to autonomous implementation and continuous cleanup.
- **Pass 2, traceability:** Mapped every guidance entry to work and acceptance evidence above. Material additions come from inspected source or are explicitly proposed behind G1–G4. Reference-repository conventions do not override broker's subcommand rule. Routine implementation proceeds without repeated permission requests.
- **Pass 3, omission challenge:** Rechecked that a clean tree is an early checkpoint, cleanup occurs throughout subsystem work, and the user's final review follows the completed remediation. Makepad remains an evaluation; production and fixture roots are traced before pruning. Deployment/live trust and material functionality retirement remain separate decisions. All seven supplied paths have a purpose in R1–R7.
- **Source limitation:** None for the user's request. Historical receipts and reference docs contain claims not rerun in this session; those remain historical evidence.

## Reference locations and provenance

Use the existing local checkouts. The placeholders below replace machine-specific drive/layout details in this potentially shared document. They are not build defaults. All locations vary by machine; repository names and relative source paths are sufficient, so no additional permission to persist private absolute paths was needed.

| Ref | Supplied location, in portable form | Purpose |
| --- | --- | --- |
| R1 | `<teamy-broker>/` | Implementation root; this plan is `docs/repository-remediation-plan.md`. |
| R2 | `<teamy-docs>/src/teamystyle.md` | Maintained requirements; follow the relevant linked chapters and repository instructions. |
| R3 | `<skills>/.agents/skills/resumable-implementation-plans/SKILL.md` | Living-plan protocol, guidance preservation and three-pass intent audit. |
| R4 | `<workspaces>/2026-11-15 teamy-broker.code-workspace` | User's existing workspace and reference discovery. It links Rust analysis to broker; it is not a request to modify all listed projects. |
| R5 | `<teamy-rust-cli>/` | Foundation and capability comparison. |
| R6 | `<Cloud-Terrastodon>/` | Curated implementation examples; inspect applicable code, not Azure-specific dependencies. |
| R7 | `<makepad>/` | Rust Android build/runtime reference for the feasibility trial. |

| Checkout | HEAD inspected | Working-tree qualification |
| --- | --- | --- |
| broker | `c1e71fafef9b82b98cfc5f0f6ae0a5fc17149338` | Existing staged, unstaged and untracked work; findings describe the working tree unless marked index. |
| Teamy Docs | `7f2aa3a4e052c81b4dc9e6177e94b7447cf2eab8` | Dirty, including `src/code-standards.md`; `src/teamystyle.md` unchanged. |
| Teamy Rust CLI | `2c90b5417a14cd2588a0b482187033aedd923024` | Dirty manifest and initializer skill; source differs from older documented template snapshots. |
| Cloud Terrastodon | `6b873f26606bc89be07d4538e71d22c0c312dbdd` | Clean during inspection. |
| Makepad | `7cf37fec5da03fd60349bbad8d21a80452e62697` | Only `libs/relax/KNOWN_GAPS.md` reported modified. |

Record a new revision and relevant local delta when consulting these sources again. Do not fetch, reset or modify reference checkouts to make them agree.

Useful specific patterns:

- TeamyStyle sections **Separate arguments, requests and operations**, **Put one type in one discoverable file**, **Keep one meaning across front ends** and **Specify observable CLI contracts** establish the intended boundaries. Read `rust-cli-template.md`, `teamystyle-template-audit.md`, `code-standards.md`, `good-cli.md`, `output-shape.md`, `logging.md`, `cancellation.md` and `repository-hygiene.md` for the affected slices. The VPN/torrent proposals are not broker requirements.
- Cloud Terrastodon `crates/source_cli/src/source_hunt_cli.rs:17` converts CLI inputs to `SourceHuntRequest`; `crates/source/src/source_hunt_request.rs:47` returns a domain report with execution dependencies supplied separately. `crates/source_types/src/source_file_extension.rs:15` illustrates constructor validation shared with a Facet proxy; copy the validation pattern, not extension-specific rules.
- Cloud Terrastodon `crates/app/src/app.rs:124` separates parsing from process startup; `crates/app/src/output/cli_output.rs:154` uses a supplied writer and checked flush. Its `AGENTS.md:70` documents cancellation-scope ownership and draining. Broker needs its own lifecycle decision before adopting these APIs.
- Makepad `tools/cargo_makepad/src/android/compile.rs:348`, `:849`, `:1352` and `:1407` show Java bootstrap, Rust `cdylib`, Java compilation and D8 packaging. `tools/cargo_makepad/src/android/java/dev/makepad/android/MakepadNative.java:18` and `platform/hal/src/backend/android/jni.rs:108` show the Java/JNI platform boundary.

## Established findings

This is a planning audit, not a completed correctness or security review of every file.

| Finding | Evidence | Consequence |
| --- | --- | --- |
| The current index is not a coherent migration checkpoint. | `handoff/build.rs` is staged for deletion while `git show :handoff/src/lib.rs` still includes generated `OUT_DIR/phone_config.rs`. The removal of that include is unstaged. | Review/stage the migration as a complete unit; staged whitespace success does not prove it builds. No index-only build was run. |
| Runtime configuration work already exists. | Untracked `src/broker/mobile_dummy/phone_profile*.rs`, typed peer/JDK values, Android `PrivatePhoneConfig.java` and `PhoneConfigStore.java`; build scripts stop generating per-phone source. | Finish and validate that work. Do not start an unrelated replacement or describe the old build-time design as unchanged. |
| Configuration has no normal CLI management route. | `PhoneProfile::load` reads runtime JSON; `src/cli/command.rs` offers pairing/status/authorization but no profile configuration. | Add discoverable typed inputs and defined persistence/reload behavior. JSON can remain storage/interchange. |
| Production phone code lives under dummy names. | `src/broker/server.rs:37` starts resident phone work; `phone/resident.rs` and `phone/transport.rs` use profile, peer, Java worker and pairing code from `mobile_dummy`. | Extract shared live capabilities before removing synthetic code. Comments in `mobile_dummy/mod.rs:1`, `pairing_server.rs:18` and `handoff/src/lib.rs:1` overstate their isolation. |
| CLI boundaries exist but are incomplete. | `src/cli/mod.rs` converts commands to requests. `src/broker/dispatch.rs:2` imports `CliOutput`; `src/protocol/request.rs` uses operation strings and a lease-ID field for several identities. | Return domain reports; retain wire compatibility through explicit conversion while introducing meaningful request/identity types. |
| Broker has template lineage and capabilities worth retaining. | `README.md` identifies source based on `99460d927c745a43285b0c5206fc2d42cec33ebc`; broker logging independently controls formats/filters and appends files. | Reconcile provenance with history, then audit capabilities. Current template source lacks some of these logging features; copying it would regress broker. |
| Some examples are required fixtures and SDK is a distinct entrypoint. | `sdk/src/lib.rs:5` source-shares protocol/SDK code. `tests/autostart_capture.rs`, `ipc_contract.rs` and `provider_supply_cancellation.rs` launch example binaries. | CLI reachability is the starting point, not sufficient deletion evidence. External SDK consumers also matter. |
| Test isolation and wiring have gaps. | Three logging tests run ordinary `status` against the default endpoint. `android/tests/PrivatePhoneConfigTest.java` has no maintained runner after deletion of `phone-config-test.ps1`. | Repair the harness before using full-suite success as a cleanup gate. |
| Historical documents disagree about current behavior. | `docs/mobile-phone-test-runbook.md` calls itself current but describes old artifact/gate state; persistent-phone notes mix operational guidance and device receipts. | Choose maintained user documents and distinguish dated receipts. Do not treat old approval text or pass counts as current authorization or proof. |
| Makepad supports Rust-led Android but retains Java/JNI. | R7 build and lifecycle code above. Broker's root `src/lib.rs` rejects non-Windows targets. | Evaluate a platform-neutral Rust component with Android adapters; do not attempt to link the current Windows root crate directly into Android. |

Before this plan was added, Git reported **33 changed tracked paths** and **48 untracked files**, including **36 untracked files under docs**. `Cargo.toml` had both index and worktree changes; the other staged path was deleted `handoff/build.rs`. Counts describe this inspection, not future work.

## Scope, retained guarantees and open decisions

The cleanup covers broker CLI/service, SDK, protocol/handoff code, Windows adapters, Android integration, pairing worker/build tooling, tests, examples and maintained documentation. Reference repositories remain references. New services, unrelated consumer changes, a general cross-platform desktop broker, deployment and publication are outside this planning request.

Keep current documented boundaries until an explicit design decision changes them: exact-use approval and fixed leases; deny/cancel/expiry behavior; generation-safe cleanup; redacted diagnostics; no implicit provider fallback; separate enrollment and per-action credential authorization; and a cooperative same-user security boundary. Debug Android permits the explicit synthetic fixture, not ordinary credential use. Existing device acceptance is limited to the documented observations and does not qualify today's source. Cleanup must not silently broaden these guarantees or claim hostile-same-user isolation.

| Gate | Decision required | Proposed starting point and acceptance consequence |
| --- | --- | --- |
| G1: supported product surfaces | Which Windows CLI/SDK workflows, phone preview/credential modes, manual probes and Android targets remain supported? What compatibility is required for known consumers? | Preserve documented reachable behavior while inventorying it. Resolve unsupported/obsolete surfaces before deleting them. Record a consumer and target matrix in 2.1. |
| G2: runtime configuration | Command names/flags; application-home ownership; read/write and redaction policy; migration behavior; when settings take effect; what happens to existing trust after peer/JDK changes. | Typed CLI inputs, validated profile persistence, explicit application/restart semantics, fail-closed invalid configuration. Do not silently rebind an enrolled identity. Close in 2.2. |
| G3: Android architecture | Shared Rust core plus existing Android shell, Makepad UI/runtime, or retained Java platform layer with less duplicated logic? | Prefer Rust-owned shared rules when the trial proves a benefit. Zero Java and replacing the pinned UKEY2 implementation are separate, unproven goals. Decide after 4.1. |
| G4: compatibility and dependency changes | Wire/profile/API compatibility policy, selected foundation/cancellation revisions, and distribution layout for pairing artifacts. | Keep explicit wire adapters initially. Compare capabilities and lifecycle behavior before upgrades. Installed binaries must not depend accidentally on a source checkout. Close each affected portion before 3.2, 3.3 or 4.2. |

Working assumptions: Windows remains the primary broker target; supported Android workflows remain usable until G1/G3 decide otherwise; the SDK remains separate unless its consumers justify a change. These assumptions permit planning, not product retirement or a framework rewrite.

## Execution order

Review pending work → isolate validation → obtain a coherent clean checkpoint → settle supported surfaces and configuration → repair and prune each subsystem → evaluate Android Rust ownership without blocking unrelated cleanup → audit remaining obsolete material → qualify supported workflows and finish documentation.

### [x] 0.1 Establish the planning baseline

**Completion notes:** Inspected current source, staged/unstaged changes, untracked implementation files, workspace membership, docs and reference code. Independent read-only reviews covered topology, reference patterns and Android feasibility. This plan is the only intended new file.

**Validation performed:** `cargo metadata --locked --offline --no-deps --format-version 1` succeeded and identified three workspace packages, 16 root examples and separate SDK/handoff targets. `rustup run nightly -- cargo fmt --all -- --check` failed on existing formatting differences (exit 1). The default-toolchain formatter also failed and warned about nightly-only options. `git diff --check` failed (exit 2: extra blank line at EOF in `handoff/src/lib.rs`); `git diff --cached --check` passed. No compile, Clippy, unit/integration test, Java/APK build, phone or live broker check was run. Existing test claims were not promoted to current results.

**Tooling note:** The sandbox failed process setup for repository commands. Read-only commands succeeded outside it. Treat missing tools/auth similarly when justified; this is not evidence of a repository defect.

**Completion criteria:** User guidance, actual source relationships, known gaps and honest validation limits are recorded in a resumable plan.

## Phase 1: review and checkpoint existing work

### [x] 1.1 Assign every pending change a disposition

**Completion notes:** All original 33 tracked changes and 48 untracked files have dispositions. Retain/fix the runtime-profile migration and its 12 new source/type files; remove its three obsolete generation/test files; retain the Facet network feature needed by typed addresses. Adopt the independent `absolute_paths` lint with import repairs rather than disabling it. Preserve all 36 original untracked documentation/evidence files (951,710 bytes) byte-for-byte under ignored `ignore/remediation-evidence/2026-11-15/docs/`; the ignored manifests record original paths and SHA-256 hashes. Merge reusable contracts into `phone-approvals.md`, `protocol.md`, `validation.md` and a rewritten `mobile-handoff.md`, removing the other 35 obsolete public copies. No private receipts were staged. The agent verified all backup hashes and 10 Markdown links. Existing source work was preserved; no commits yet.

**Work:** Re-read HEAD/index/worktree separately. Group the runtime-profile migration (including staged build-script deletion, Rust callers, Android settings and tests), the independent lint change, and documentation/evidence. Mark each path retain/fix, redundant, historical evidence, private local evidence, or unresolved. Review untracked evidence before any broad staging; some documents contain device/network identifiers and local delivery receipts. Preserve useful decisions and failure cases without publishing raw machine records.

**Validation:** From broker root, run `git status --short --untracked-files=all`, `git diff --cached`, `git diff`, and `git ls-files --others --exclude-standard`. Inspect the index's `handoff/src/lib.rs` alongside the deleted generator. Record per-group ownership and dependency order here, without copying private values.

**Completion criteria:** Every pending path has a reasoned disposition. No reset, stash, blanket discard, directory purge or bulk commit has hidden unfinished work. The current migration is understood well enough to review as one functional change.

### [x] 1.2 Make baseline verification repeatable

**Completion notes:** Logging tests now invoke `synthetic_logging_cli`, which supplies an SDK client connected to its own random status-only pipe through the shared CLI entrypoint. The production endpoint policy is unchanged, and fixture success requires receipt of the request. Android's maintained runner now executes profile validation and durable-save policy tests. Java worker host tests resolve a development JDK without reading the user's phone profile. `cargo check --offline --workspace --all-targets` and `cargo build --offline --workspace --bins --examples --features extended_observability` passed; `cargo test --offline --workspace --features extended_observability -- --test-threads=1` passed with all three logging tests enabled and four explicit native/device tests ignored. Android host tests and compilation of all Android Java sources against existing API 35 inputs passed. Subsequent import normalization still requires final lint/build validation.

**Work:** Give the three live-endpoint logging tests an owned isolated endpoint/server fixture. Keep their intended logging assertions. Wire `PrivatePhoneConfigTest` into the maintained Android host runner. Check all harnesses for hidden live endpoint, installed-binary, example-binary and external-tool dependencies. Record missing prerequisites rather than silently installing them. Review `check-all.ps1` and the dated candidate scripts before reusing them as an umbrella command.

**Validation:** Use V1 below for compilation and the currently isolated suite. Run `cargo test --offline -p teamy-broker --test log_sinks --test log_span_timings -- --test-threads=1` after isolation, and `./android/check-java.ps1` after wiring the profile test. Prove these tests no longer contact the user's default resident endpoint. Preserve explicitly ignored native/device tests.

**Completion criteria:** Normal tests own their processes/endpoints/state, configuration regressions are executed by a maintained runner, and the suite runs whether the user's broker is present or absent. Remove the three temporary skips only after this is established.

### [~] 1.3 Finish the pending migration and make coherent commits

**Completion notes:** Rust listener binding, source admission and worker launch now share the verified profile; profile changes invalidate ongoing work. Snapshot regressions cover changed peer/JDK values, malformed and removed profiles. Android configuration uses committed SQLite rows with read-only legacy preference fallback, and rejects changed/missing settings while paired without clearing trust. Host tests cover policy/storage failures; Android device durability is unverified. Source compilation caught and fixed two lambda catch-name collisions in the pending Activity change.

Checkpoint validation on 2026-11-15: full strict workspace Clippy with all targets/features passed; nightly workspace formatting passed; all 14 lint-policy cases passed; all bins/examples built with `extended_observability`; the complete workspace suite passed with 219 tests passed, zero failed, four explicitly ignored and no logging skips. The rebuilt official Java peer had already passed 58 host tests; Android host tests and complete Java source compilation passed. Execution output is retained locally under ignored remediation validation evidence.

Review also repaired interrupted descriptor publication (stage, sync, publish before scheduler registration), repeated cleanup, wrapped missing-task classification, parse-before-process setup, invalid-Unicode argument rejection, public build identity and joined/cancellable human comparison input. Regressions use temporary metadata, isolated pipes and pure exception fixtures, without native credential or scheduler effects. Confirmed dead cache/shape helpers, obsolete handoff pairing/status examples and the dated candidate script were removed. All source additions for subsequent config/JNI work remain outside this initial checkpoint until integrated and validated; the final clean-tree requirement still applies.

Initial supported-surface decision (G1): preserve documented CLI, separate SDK, reachable phone workflows and meaningful interoperability fixtures. Remove unused template helpers and a permanently disabled historical demo. Removed `src/cli/facet_shape.rs`, `src/paths/cache.rs`, unused cache/global-home exports, `examples/synthetic_status.rs`, the three-file `mobile_handoff_pairing` example, its constant-false accept wrapper, and the dated `android/check-pairing-candidate.ps1`. Caller searches found no remaining dependencies. `handoff` and persistent enrollment wire strings remain because production uses them. Metadata should now expose 15 examples (original 16, plus logging, minus two removed examples).

**Work:** Fix defects found in 1.1 within the existing migration's scope; include all necessary new files. Keep the generator deletion with removal of its consumers. Preserve profile-schema compatibility and existing Android identity/settings. Correct formatting and whitespace in the reviewed groups. Resolve every untracked item: useful repository material, explicitly selected local/private storage, or demonstrated obsolete material. Create reviewable commits in dependency order once implementation is underway; do not commit broken intermediate staging or unrelated evidence.

**Validation:** Run V1 and V2 as relevant; run the existing `phone_profile` tests. Inspect `git diff --cached --check` and the complete staged diff for each commit, then validate the committed source, not only the larger dirty tree. After all reviewed groups are accounted for, `git status --porcelain=v1 --untracked-files=all` must be empty. Record commit IDs and any remaining limitation here.

**Completion criteria:** A clean, reproducible checkpoint contains the retained work; no useful user work was lost. This is a prerequisite to the broader refactors, not completion of remediation.

## Phase 2: define supported workflows and configuration

### [ ] 2.1 Map the whole repository to supported behavior

**Work:** Close G1. Start with `main` → entrypoint → CLI → dispatch → server/SDK, then enumerate the SDK public API, Android Activity/service, Java worker, Cargo build scripts/targets, scheduler cleanup entrypoint, examples and test harnesses. For every first-party module, test and example, record owner, callers, purpose and disposition. Check known local consumers before changing exported SDK/wire behavior. Classify manual probes separately from automated fixtures. Include unchanged credential engine, storage and Windows code in the review inventory.

Build a capability matrix against R2/R5/R6: typed parser/help, build identity, parse/process separation, checked output, independent logging, cancellation, lint policy and test isolation. Reconcile README's original template provenance with history. Preserve broker's stronger logging/failure behavior; do not assume newer template source satisfies every documented requirement.

**Validation:** Use `cargo metadata --locked --offline --no-deps --format-version 1`, `rg --files src sdk handoff android pairing examples tests`, and targeted `rg -n` caller searches. Follow dynamic process launch strings, script invocations and manifest entrypoints as well as Rust imports. A deletion proposal needs caller/consumer evidence and replacement test coverage.

**Completion criteria:** Every owned subsystem has a review/disposition entry, and the supported-surface/target matrix has an explicit decision for every currently advertised workflow. No large unchanged subsystem is exempted because the diff did not touch it.

### [ ] 2.2 Specify configuration ownership and effects

**Work:** Close G2 with exact discoverable CLI flags and typed request/result models. Choose profile location ownership, bounded validation, atomic replacement/recovery behavior and migration persistence. Reading should retain the current no-rewrite behavior unless deliberately changed. Decide whether a running listener reloads safely or requires an explicit apply/restart, and how existing certificate/trust identity relates to a changed peer profile. Audit the separate loads in `phone/resident.rs` and `mobile_dummy/java_pairing_worker.rs`; define one consistent configuration snapshot for each operation.

Candidate command vocabulary is `phone-configure`, `phone-config-show` and `phone-config-validate`, matching current flat phone verbs. These names are proposals, not available commands. Ordinary peer/JDK settings must be available as Figue arguments; a JSON import may remain optional. Define redacted results and safe diagnostics without echoing private values by default.

**Validation:** Walk through missing/invalid profile, valid initial setup, v0/v1 inputs, unsupported version, settings change during pairing, enrolled peer replacement, interrupted write and stale listener cases. Map each to expected CLI result, disk effect, listener/trust effect and test layer. Inspect generated leaf-help support before promising help content.

**Completion criteria:** A written behavior table resolves all G2 decisions and names tests; configuration management is distinguishable from enrollment and credential authorization. Existing grants cannot silently move to a new peer.

### [ ] 2.3 Implement runtime configuration through the CLI

**Work:** Implement the agreed commands in individual `src/cli/<command>/teamy_broker_<command>_cli.rs` files with `mod.rs` re-exports. Move configuration ownership out of synthetic modules. Keep versioned schema decoding, meaningful peer/JDK types and invariant checks; tighten ownership where public mutation can create invalid combinations. Add domain operations for validate/load/save/apply as selected in 2.2, then wire CLI and service to them. Retire the legacy environment helper only after identifying its callers and migration path.

**Validation:** Run `cargo test --offline -p teamy-broker phone_profile` and the new focused CLI/integration tests, plus `./android/check-java.ps1`. Build the source binary and inspect root and each new leaf's `--help`. Use an isolated application home to change synthetic profiles twice with the same executable/APK, proving no rebuild and no live trust change. Test failed writes, redaction, migration, unsupported schemas, snapshot consistency and the selected apply/restart behavior. Finish V1.

**Completion criteria:** Users can discover and supply ordinary phone settings through CLI help; valid changes work without recompilation; invalid/partial changes cannot activate a listener or reuse mismatched trust. Existing profiles receive the documented migration treatment.

## Phase 3: repair ownership and review core behavior

### [ ] 3.1 Separate live phone components from fixtures

**Work:** Move configuration, exact-peer policy, enrollment worker adapters and resident transport helpers into clearly named phone modules. Keep UKEY2 and current transport behavior while moving code. Separate synthetic queue/session/adapters into deliberate test/example support based on 2.1. Review `handoff` ownership: production pairing types and synthetic secret/constants currently share a crate. Choose a small shared module/crate boundary only where its consumers justify it. Fix misleading comments as each move lands.

**Validation:** Build all workspace bins/examples; run `cargo test --offline -p teamy-broker-handoff`, `cargo test --offline -p teamy-broker --test mobile_dummy_contract --test mobile_tls_contract`, and relevant resident/pairing tests through V1/V2. Search every renamed module and process entrypoint. Verify normal serve still wires the intended phone runtime, and test support cannot select a production provider accidentally.

**Completion criteria:** Live phone behavior has production ownership and discoverable names; remaining dummy names describe genuinely synthetic behavior. Android package/application IDs, component names, signer and stored identity are unchanged by a Rust module move. Any later Android rename needs G3/G4 migration evidence.

### [ ] 3.2 Establish typed operations and results one command at a time

**Work:** Close G4's wire/API compatibility policy. Start with redacted phone status: CLI arguments → typed request → operation → typed domain report → CLI renderer. Move `CliOutput` ownership out of broker operations. Let argument types own their conversions when appropriate. Introduce distinct attempt/lease/request identities and closed operation types where they prevent current confusion; keep explicit wire conversion to preserve accepted protocol versions. Split large type-bearing `mod.rs` files, including phone `Controller`, into discoverable files with inherent implementations beside their types. Extend the proven pattern across the command set.

**Validation:** Run `cargo test --offline -p teamy-broker --test cli_contract --test ipc_contract --test failure_contract`, then the affected phone/control tests. Prove old accepted frames and SDK callers remain compatible or reject with the explicitly chosen version policy. Verify text/JSON/CSV restrictions, writer/flush failures and redaction with the same domain result. Check root/leaf help from the built binary without starting a broker.

**Completion criteria:** Broker operations no longer construct CLI presentation objects; request identities cannot be confused through a reused lease-ID slot inside domain code; every supported command has a reviewed conversion and result path. On-wire changes are intentional and tested.

### [ ] 3.3 Review engine, lifecycle and platform effects

**Work:** Use 2.1's inventory to review credential acquisition/approval, provider arbitration, caching, lease deadlines, cleanup/persistence, SDK autostart and process ownership, Windows handles/UI, phone revocation and secret buffers. Record each subsystem's invariant, concrete issues, bounded repair and proof. Review human interaction budgets separately from transport/shutdown timeouts, including the current five-second pairing-confirm stdin read. Preserve resource ownership, generation safety and committed state under cancellation.

Close G4's foundation/cancellation choice independently of module movement. Broker resolves `teamy-cancellation` 0.3.1; inspected template and Cloud Terrastodon use different later APIs. Review those APIs before changing the pin. Replace polling/worker patterns only with explicit ownership, cancellation, completion and drain semantics. Audit dependency necessity, feature/lint inheritance across all three packages, and startup/help/version behavior. Adopt the applicable TeamyStyle requirements without importing Cloud Terrastodon's entire architecture.

**Validation:** Run affected `broker_contract`, `provider_supply_cancellation`, `phone_control_deadlines`, `shutdown_ipc`, `shutdown_status`, `autostart_capture` and failure tests, then V1. Add focused tests only for meaningful gaps: denied/replayed/stale approval, cancellation with partial effects, cleanup failure/retry, expired leases, independent daemon survival, handle/worker cleanup and redacted remote failure. Use owned synthetic fixtures; native persistence and UI acceptance remain separately identified in the matrix below.

**Completion criteria:** Every subsystem from 2.1 has a completed review or an explicit supported-scope limitation, with repaired defects and evidence. Dependency/version changes have capability and lifecycle justification. Passing style checks alone cannot close this task.

## Phase 4: evaluate and implement Rust Android ownership

### [ ] 4.1 Run a bounded shared-Rust trial

**Work:** Extract one pure behavior, preferably profile validation or a protocol/state transition, into a platform-neutral Rust component with an Android-callable boundary. Compare shared Rust plus the current Android shell with a small Makepad integration only if its UI/runtime capabilities solve a named broker need. Retain existing algorithms, pairing/trust behavior and package identity during the trial. Root broker's Windows-only crate is not the shared component. Keep replacement of the desktop Java UKEY2 worker as a separate interoperability/cryptography decision.

**Validation:** Execute the same accepted/rejected fixtures against the existing behavior and shared Rust, then build for one explicitly selected Android ABI. Record exact toolchain, NDK/API level, commands, dependency/license cost and package size. Add emulator/device evidence for the boundary actually exercised; host JVM tests cannot establish Activity/service/Keystore behavior. Resolve manifest minimum 29 versus packaging `--min-api`/`--min-sdk-version` 26 by inspecting packaged metadata before declaring an Android floor.

**Completion criteria:** Close G3 with measured implementation/build costs and named parity gaps. Choose shared Rust with adapters, Makepad, or a justified retained Java layer. A compiled sample alone does not justify a rewrite, and an infeasible trial is a useful recorded outcome.

### [ ] 4.2 Apply the selected Android boundary in reviewable slices

**Work:** Follow G3; migrate shared rules incrementally where the trial supports it. Define Android adapters for lifecycle, foreground service/notifications, Keystore, TLS/signing, input and cancellation. Keep one domain meaning across desktop/phone, while preserving platform-owned key handling and explicit user start/stop. Decide G4's worker distribution layout so installed broker runs without `CARGO_MANIFEST_DIR`/source-tree artifact assumptions. If Java remains, document exactly which responsibilities justify it and remove only duplicated logic replaced by the shared component.

**Validation:** Use V2 plus the chosen Android build commands recorded in 4.1. Test pause/recreation, process death, service stop/restart, stale callbacks, input clearing, reconnect, revocation and interrupted enrollment. Compare against existing protocol/official-peer tests. Verify nonexportable key and required hardware properties on supported devices, and preserve or explicitly migrate application/signing/identity state. Installation and live trust changes require the concrete device acceptance scope to be arranged separately.

**Completion criteria:** Selected Android workflows meet their declared parity matrix; shared rules have one owner; remaining platform code has a documented purpose. No claim of all-Rust implementation, hardware support or delivery reliability exceeds observed evidence.

## Phase 5: audit cleanup completeness and qualify the result

### [ ] 5.1 Prune code, examples and tests using the inventory

**Work:** Perform removals during the subsystem slices as U18 directs, then use this task to check every remaining candidate from 2.1. Keep essential subprocess fixtures such as `synthetic_broker`, `synthetic_autostart_consumer` and `synthetic_op_wait` until their harnesses are deliberately replaced. Move useful manual diagnostics to a discoverable location with purpose and invocation; remove obsolete experiments and unused dependencies. Consolidate tests by the user-visible behavior or failure they prove, not by age or LLM origin. Retain independent interoperability, race, lifecycle and regression evidence.

**Validation:** Re-run Cargo target metadata, updated fixture references and affected tests after each bounded removal. Run V1/V2 once the removal group is complete. Confirm packaging scripts, SDK public use, named Java entrypoints and cleanup commands still resolve. Record disposition counts and representative evidence here, not another permanent inventory dump.

**Completion criteria:** Every retained example/test/module serves a supported workflow, regression or named diagnostic purpose; every removed surface has evidence and any needed compatibility treatment. A search returning no direct Rust caller is insufficient on its own.

### [ ] 5.2 Consolidate maintained documentation and historical decisions

**Work:** Keep a short maintained entry route: README for purpose/quick start, architecture/module ownership, configuration/phone operations, protocol/threat model, and validation/support limits. Reuse existing files where suitable. Fold still-valid decisions into the owning topic; retain only historical evidence needed to explain a decision or unresolved limit, with date/revision/status. Review `docs/archive`, mobile receipts, TLS evidence and SDK-license evidence individually; preserve required license/provenance material. Remove stale current-runbook links and old operational approval/state claims. Move private device receipts to an explicitly selected local location or redact them; do not invent a storage destination or duplicate their values here.

**Validation:** Search docs for references to removed commands/modules, build-time phone fields, old dummy-only assertions and dated claims presented as current. Resolve relative links and verify all documented commands against current source/help. Read the maintained path end-to-end as a new user. State unavailable evidence and unresolved support limits explicitly.

**Completion criteria:** One discoverable document owns each operational topic; current instructions agree with code; historical receipts cannot be mistaken for current state or permission. The remediation plan remains the living execution record rather than spawning competing plans.

### [ ] 5.3 Complete workflow acceptance and final clean checkpoint

**Work:** Exercise the declared supported workflows through the CLI, SDK and selected Android integration. Record source revision, commands, target/build identity and observation limits. Verify distribution from outside the source checkout, including required pairing artifacts. Update documentation and this plan, then commit the reviewed result in coherent slices. Any installation, daemon replacement, signing, publication or device trust operation is a distinct concrete action; planning/source cleanup does not imply it occurred.

**Validation:** Run final V1/V2, the target matrix and selected interactive acceptance. Inspect final diffs for accidental private records. `git status --porcelain=v1 --untracked-files=all` must be empty after the final commits. Record every deliberately unavailable target/check as unverified; do not mark supported behavior qualified solely because a historical receipt exists.

**Completion criteria:** All release truths below hold, or the plan names the remaining work and stays active.

## Validation commands and target matrix

Run from broker root with pre-existing Rust/Java dependencies. Use the current source tree and isolated temporary state. Serialize heavy builds. On disk-space errors, wait for the user. Commands below are planned validation unless 0.1 explicitly records their execution.

**V1: Rust baseline.** Build example fixtures before the suite. Logging tests now own their endpoints, so no temporary logging skips are needed. Ignored native/device tests remain excluded from the normal suite.

```powershell
rustup run nightly -- cargo fmt --all -- --check
cargo clippy --offline --workspace --all-targets --all-features --no-deps -- -D warnings
cargo build --offline --workspace --bins --examples --features extended_observability
./check-lint-policy.ps1
cargo test --offline --workspace --features extended_observability -- --test-threads=1
git diff --check
git diff --cached --check
```

**V2: current pairing/Android host validation.** Pinned JAR/proto dependencies must already match `pairing/java/dependencies-lock.json` and `vendor-lock.json`; scripts should fail on missing/mismatched inputs. The TLS check uses isolated loopback, not the phone's tailnet.

```powershell
./pairing/java/build.ps1 -Test
./android/check-java.ps1
cargo test --offline -p teamy-broker-handoff
cargo test --offline -p teamy-broker --test mobile_tls_contract
cargo build --offline -p teamy-broker --example mobile_tls_probe
./android/check-tailnet-transport.ps1
```

| Target/workflow | Current planning status | Required proof before qualification |
| --- | --- | --- |
| Windows CLI and resident broker | Existing product; current build/test health unverified | Help/version and output isolation; source-built CLI/IPC flows; approval/lease/cancel/recovery tests; no source-tree runtime dependency. |
| Windows SDK and known consumers | Existing public surface; source-sharing layout | Compatibility and consumer compilation; isolated acquisition/autostart/cleanup; EOF and daemon ownership tests. |
| Current Java UKEY2 and Android host logic | Existing adapters; tests not run in this audit | Official-peer/interoperability and malformed/replay tests; runtime-profile test included; host lifecycle/policy tests. |
| Android debug/release packages and selected device/API/ABI | Existing workflows; support floor and release acceptance require G1/G3 decisions | Inspect packaged metadata/signing; configuration without rebuild; lifecycle, notifications, key policy and enrollment recovery on chosen target. Debug stays synthetic-only. |
| Native Windows storage/Task Scheduler/UI and live credential/phone workflows | Separate effectful or interactive acceptance | Reviewed isolated native tests (`cargo test --offline --test native_backend -- --ignored --test-threads=1`) when selected; human-observed UI/device behavior; chosen live credential scope. Host tests are insufficient. |
| New shared-Rust Android component | Proposed; no implementation or support claim | Selected ABI compile, cross-language fixtures and device-boundary evidence from 4.1–4.2. |
| Other desktop platforms | Root crate explicitly Windows-only | No support expansion implied. Any portability work requires its own decision. |

## Risks and controls

| Risk | Control |
| --- | --- |
| A clean Git tree conceals lost work or a broken staged snapshot. | 1.1 path dispositions and 1.3 coherent committed-source validation; no blanket reset/discard. |
| Removing dummy/example code breaks product behavior or tests. | 2.1 inventories all entrypoints and consumers; 3.1 extracts live code before 5.1 removes anything. |
| Configuration reload changes the peer/JDK/trust during an operation. | G2 snapshot and trust policy; failed-write, mid-operation change and stale-generation tests in 2.3. |
| A module or Android package rename changes installed identity or trust. | Separate Rust organization from Android application/component/signing migration; 4.2 parity and recovery evidence. |
| A framework/template/dependency replacement loses broker behavior. | Capability comparison, one-command typed pilot, separate cancellation migration and bounded Android trial. |
| Legacy test counts or local receipts create false confidence or expose machine details. | Fresh V1/V2 results, explicit device limits, 5.2 provenance/publication review. |
| Cleanup weakens credential or persistence boundaries. | 3.3 invariant review, synthetic regressions and separately scoped native/interactive validation. |

## Overall completion criteria

- [ ] Existing work is accounted for and the final Git working tree/index are clean through reviewed commits, with no lost changes or private evidence accidentally published.
- [ ] Every owned subsystem has been reviewed against supported workflows and TeamyStyle; retained deviations have concrete reasons and evidence.
- [ ] CLI configuration is discoverable and works without rebuilding; production phone ownership is clear; SDK, daemon and Android adapters share the intended domain semantics.
- [ ] The selected Android approach has recorded feasibility/parity evidence; obsolete code, examples, tests and documentation have reasoned dispositions.
- [ ] Maintained docs and acceptance results agree with the current source revision. Unverified targets and interactive limits remain explicit, and no required remediation work is silently marked complete.
