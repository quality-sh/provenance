# Provenance 0.2.4 onboarding: research and draft implementation plan

This plan covers the remaining first-use work after the plumbing and porcelain work. It is a proposal for review. It authorizes no runtime change, release, or website edit. The target release is 0.2.4. The planning base is `f76084c69b2ded43b9900e5ee5f630d62cb5b692` on `origin/main` (fetched 2026-09-23). The original brief cited earlier main `f9a70076c52507e8b009ca89e679f9b417ef3ce8`. The forthcoming porcelain foundation is `2a407147c41c1de911dabfa31275b89e6b18a7f2` on `feat/porcelain-search-recovered` (PR 317, CI run 35841011269). Prepare onboarding changes now, but refresh and merge runtime changes only after PR 317 lands. This plan does not import code from that branch.

## Desired result

The recorded decisions call for bare `provenance init` defaults, a clear first-use result, and this final line: “Have your agent run provenance prime to get acclimated.” The historical defect ticket also calls for init guidance when graph state is absent and no partial state after a rejected write. The proposed implementation adds a no-change result on repeat runs and a coherent root-selection policy for nested invocations. Explicit target selections remain effective. `create-provenance` keeps its automatic package install and local engine call. Successful dictionary acquisition stays out of normal output. A failed acquisition keeps actionable guidance and does not fail an otherwise successful init.

The plan keeps file and release work separate. It specifies cases for implementation PRs, a performance investigation, and a website handoff. It does not prescribe a new installer channel or product policy.

## Evidence and current state

The classifications below describe `origin/main` at the pinned SHA unless a row names another source. Source inspection proves code structure, not an end-to-end runtime result. Historical reports need fresh reproduction after PR 317.

| Area | State | Evidence and implication |
| --- | --- | --- |
| Dictionary acquisition | Done in PR 249, merged as `5a422e8c`; retain | [`ste_onboarding.rs`](../../crates/provenance-cli/src/ste_onboarding.rs) selects project reuse, explicit PDF, or the official asset at lines 26–70. It warns and continues on download failure at lines 45–50 and 264–267. `--ste-onboarding` is gone. Do not rebuild this flow. |
| Native init result | Remaining on main; pending implementation in PR 271 | [`repo.rs`](../../crates/provenance-cli/src/handlers/repo.rs) commits then calls `dictionary.print_message()` at lines 120–170. Main has no result inventory or agent closer. [PR 271](https://github.com/quality-sh/provenance/pull/271), head `ac4f3fbbce9a01ec8675ff224d3f8b6679b814bd`, has a result inventory, quiet handling, no-change line, and tests. Its numbered next steps and successful dictionary report conflict with Ben's later accepted direction. Reconcile that open PR; do not start a duplicate summary implementation. |
| Native bare init | Remaining, static finding | [`cli.rs`](../../crates/provenance-cli/src/cli.rs) requires `--path` at lines 52–55. [`repo.rs`](../../crates/provenance-cli/src/handlers/repo.rs) rejects a new repository without `--scope` at lines 53–70. The accepted bare form needs defaults while explicit selections and repeat-run manifest fields remain effective. |
| Cargo adapter | Existing path with presentation and root work | [`cargo_init.rs`](../../crates/provenance-cli/src/handlers/cargo_init.rs) uses Cargo metadata's `workspace_root` and selected package prefix at lines 17–38. Existing Rules cover package selection, exact SDK dependency, and rollback. PR 271 still prints a Cargo-specific line and dictionary fragment, so shared presentation needs a channel case. |
| TypeScript adapter | Install and init done; presentation and discovery remain | [`initializer.js`](../../packages/create-provenance/src/initializer.js) selects a manager, installs the exact package, resolves the installed local engine, then calls init at lines 52–100. [`create-provenance.mjs`](../../packages/create-provenance/bin/create-provenance.mjs) adds a generic readiness line at lines 20–33. Keep the stale-global sentinel and packed-install checks. |
| Existing graph root helper | Partial, static finding | [`layout.rs`](../../crates/provenance-store/src/layout.rs) lines 77–85 locates an ancestor with `.git` or a Provenance manifest. Most CLI read handlers still construct `ProvenanceLayout::new(repo)` directly. A policy and one shared selection seam are needed before adapting calls. |
| Nested read defect | Historical, unconfirmed on final baseline | `provenance-ekec` records a 2026-09-09 reproduction on build `4b644b61`: a nested read returned `[]` and created a nested cache. Reproduce after PR 317 before claiming it remains broken. Include reads from both root and nested paths and check for no nested `.provenance`. |
| Missing-state write defect | Historical, unconfirmed on final baseline | `provenance-xxb0` records a 2026-09-12 write that left a shard without a manifest. The redesigned write path may differ. Test current public write paths before selecting a guard change. Do not add one guard per handler. |
| Dictionary latency | Historical, unconfirmed on final baseline | `provenance-3ges` records two time limits on build `4b644b61` with an obsolete init flag. Current code holds a shared lock and imports PDF bytes under that lock in [`ste_onboarding.rs`](../../crates/provenance-cli/src/ste_onboarding.rs) lines 136–174. This identifies stages to measure; it does not prove which stage is slow now. |
| Site guide | Separate repository handoff | `provenance-85ay` requests an Agents tab. Its commands must use the final shipped 0.2.4 surface. No website file belongs in this PR. |

PR 271 passed its own CI at its pinned head, including CLI tests and Rule Coverage. That proves its historical branch checks, not that its current content matches the later output decision or the final porcelain baseline. PR 317 passed its pinned CI but remains a separate merge prerequisite.

## Graph and decision map

The graph additions are `source_e4s3_onboarding_brief_2026_09_23`, `req_e4s3_bare_native_init`, `req_e4s3_init_result`, `req_e4s3_missing_state_guidance`, `topic_e4s3_project_root`, and open `question_e4s3_mixed_root_precedence`. The source points to the [evidence appendix](#evidence-appendix), which separates Ben's recorded decisions, research recommendations, defect reports, and code findings. The current user instruction authorizes this draft plan and graph correction for release 0.2.4. It does not ratify each implementation proposal. The three active Requirements retain only the historical obligations that the appendix supports. The project-root policy and command applicability remain open. No new Rule is needed yet: later implementation can refine exact atomic obligations after those decisions and the PR 317 interface are known.

Reuse these graph obligations during implementation:

| Planned behavior | Existing graph anchor or Rule | Bead |
| --- | --- | --- |
| Init plans project writes, validates state, and rolls back owned changes | `req_init_installs_skills`; `rule_init_plans_all_project_writes`, `rule_init_plan_rejection_preserves_targets`, `rule_init_validates_planned_repository`, `rule_init_apply_rolls_back_owned_changes` | `provenance-1azy`, `provenance-xxb0` |
| Bare init defaults and one result | `req_e4s3_bare_native_init`, `req_e4s3_init_result` | `provenance-xxb0`, `provenance-1azy` |
| Missing-state guidance and rejected writes | `req_e4s3_missing_state_guidance`; the existing init validation and rollback Rules remain in force | `provenance-xxb0` |
| Selected Cargo package and SDK | `rule_cargo_init_selects_workspace_package`, `rule_cargo_init_uses_package_directory`, `rule_cargo_init_preserves_sdk_dependency`, `rule_cargo_init_restores_owned_files` | `provenance-tqj.5` |
| TypeScript package and local engine | `rule_typescript_initializer_installs_dev_dependency`, `rule_typescript_initializer_selects_package_manager`, `rule_typescript_initializer_validates_project`, `rule_init_typescript_local_command` | `provenance-j4yq` |
| Automatic dictionary and failure warning | `req_ste_local_dictionary_acquisition` and active `rule_ste_dictionary_agent_acquisition`, `rule_ste_dictionary_asset_fallback`, and `rule_ste_dictionary_import_reuse`. The attribution and claim-scope Rules are deprecated. | `provenance-3ges`, `provenance-1azy` |
| Root selection across init and reads | `topic_e4s3_project_root` and `question_e4s3_mixed_root_precedence`; related existing SDK discovery requirements and Rules stay in force | `provenance-tqj.5`, `provenance-ekec` |

The interactive dictionary, attribution, and claim-scope Rules are already deprecated. PR 249 merged automatic acquisition at `5a422e8c261f8ceff7d42bf4d31688d933cc2ef6`, with `rule_ste_dictionary_agent_acquisition` and `rule_ste_dictionary_asset_fallback` bound in code. Approved `res_ste_onboarding_auto_download_only` records the move of the ownership notice from init output to `LICENSE`; PR 249 also updates `README.md`. The earlier plan's reference to attribution and claim-scope Rules as current obligations was a plan-to-graph mismatch. The graph and PR 249 agree on their deprecated status; no further drift is established here. Do not infer graph validity from the presence or absence of code bindings; the canonical coverage scan makes that report.

## Proposed implementation sequence and file ownership

Each item is a bounded future change. The owner must refresh the exact files against merged PR 317 before implementation. The phase order protects the shared init and CLI files from concurrent edits. Independent investigation and website writing can proceed while runtime code waits for the foundation.

### 1. Reconcile the open init-summary PR (`provenance-1azy`)

Work with the owner of [PR 271](https://github.com/quality-sh/provenance/pull/271) on that branch or its direct successor. Keep its useful planned-change inventory, quiet routing, no-change result, and targeted tests. In `crates/provenance-cli/src/init_summary.rs`, `handlers/repo.rs`, `handlers/cargo_init.rs`, and `ste_onboarding.rs`, replace the numbered command block with the accepted final sentence on every successful init, including a no-change run. Hide successful dictionary acquisition details from normal output. Keep the warning and retry path for failure. Make the short introduction explain Provenance and distinguish created, changed, and no-change results. Decide the warning stream and quiet behavior by testing the existing global `--quiet` contract; do not let quiet suppress actionable failure guidance without an explicit decision. Preserve the dictionary attribution in the existing LICENSE and supported product text; avoid a new conformance claim.

Automated acceptance: CLI cases cover first init, changed existing files, true no-op, `--quiet`, plain redirected output, dictionary success paths, and acquisition failure. Inventory entries match actual bytes or links changed after publication. Failure paths leave owned files unchanged or restore them. Run the relevant CLI and Rule Coverage jobs in CI; no local Cargo build is needed for this plan. Manual review: compare real output from native and Cargo entry points with Ben's sentence and confirm the last normal line is exact. This is the only planned owner for native summary code.

### 2. Select one project root and finish bare native init (`provenance-tqj.5`, `provenance-ekec`, `provenance-xxb0`)

Resolve `question_e4s3_mixed_root_precedence` before changing implicit selection for an overlapping nested Cargo and JavaScript workspace. Proposed starting policy: an explicit repository path remains exact; an existing containing Provenance manifest takes priority for implicit reads; a new bare init uses the current project root after the ecosystem adapters apply their selected package and workspace inputs. The mixed-workspace case remains a proposal until Ben chooses its precedence. A nested repository with its own manifest must not silently retarget the parent. Record the chosen policy in the graph before code changes.

Put root selection at one seam shared by native init and native graph reads. Candidate files are `crates/provenance-store/src/layout.rs`, CLI dispatch or a small root-selection module, `handlers/repo.rs`, and the read handlers that currently pass `repo` straight to `ProvenanceLayout::new`. Use a small interface that accepts the explicit target, invocation path, and operation intent and returns a selected root or an actionable error. Keep the manifest itself and scope selection distinct from root selection. Cargo and TypeScript adapters must pass or receive the same selected root while preserving `--package`, `--package-manager`, `--path`, `--scope`, and `--path-prefix`. Avoid a broad rewrite of every handler before proving the common read seam. Keep each Rust file below 500 lines.

Make omitted native `--path` equivalent to `.` and a new `--scope` default to `default`, as requested in `provenance-xxb0`. An explicit path remains the selected target. For an implicit path, the proposed project-root selection may resolve that starting directory to a containing project. This policy still needs a decision, especially for mixed workspaces. The proposal keeps existing manifest scope and actor fields when options are omitted. A command that requires initialized graph state reports how to run init if no manifest exists. The historical ticket also names silent empty results from `prime` and `health`; whether `prime` remains usable as general guidance without a graph, and what it must say then, remain open. Reproduce the historical missing-state write and nested-read cases on the final PR 317 baseline first. If the redesigned write gate already rejects safely, keep it and add only the missing guidance. If a gate fails, fix the canonical state access seam, not each command in isolation.

Automated acceptance: table-driven fixtures cover a bare directory, nested Cargo package, JavaScript workspace, mixed Rust and TypeScript project, two nested manifests, explicit path, explicit scope, selected Cargo package, and selected manager. Root and nested reads return the same known Rule when they target one project. A nested read creates no nested `.provenance`. A rejected uninitialized write leaves no shard or cache in the wrong project. Init reruns do not change manifest fields or files without an input change. The relevant CLI, store, Cargo, and TS fixtures run in CI. Manual review: inspect each selected root and error text in a real shell. If mixed precedence remains open, land only the cases whose meaning is settled and keep that case gated.

### 3. Align `create-provenance` output (`provenance-j4yq`)

After phase 1's engine presentation has landed, change `packages/create-provenance/bin/create-provenance.mjs` and, if needed, the small public result from `src/initializer.js`. Preserve the existing install, local engine lookup, exact version, package manager order, and stale-global sentinel. Let the engine own the project file inventory and final agent handoff. Print package installation context only if it adds a distinct fact, and avoid a second generic success line after the engine result. Do not restore `--ste-onboarding` or add a manual dictionary prerequisite.

Automated acceptance: unit cases cover every supported manager, explicit overrides, local engine selection, init failure, and one nonduplicated result. Packed-install CI runs a real install and init, then reruns init and checks both output and state. Retain the stale-global sentinel and process-failure tests. Manual review: inspect a plain terminal and redirected output from `npx --yes @quality-sh/create-provenance` on a disposable project.

### 4. Measure dictionary acquisition before selecting a fix (`provenance-3ges`)

Use the current init interface and a CI-built artifact or a compatible downloaded CI artifact. Record elapsed time by stage: shared-cache lock wait, asset request, PDF import, index write, and project publication. Compare cold cache, warm cache, an existing project dictionary reference, and explicit `--ste-pdf`. Use a loopback asset or controlled fixture for repeatable CI tests; keep any real network observation distinct. If the measurements show a material delay, propose a narrow fix with a regression case. Do not set a new time budget from the old timeouts, print internal stages on normal success, or change the warning-and-continue contract. This investigation can close with evidence that no current regression remains.

### 5. Prepare website handoff (`provenance-85ay`)

Provide the website owner a short Agents-tab specification after the CLI text and porcelain names are final. The tab sits beside Cargo and TypeScript installation. It explains Provenance, shipped install and init paths, actual project files created or changed, the first graph action, Rule bindings and checks, and further guidance. Its agent handoff ends with Ben's accepted line. It does not show dictionary internals on the happy path or advertise curl or PowerShell installers before release. Track website implementation in its own repository; this plan changes no website files.

## Release and optional extension

Version 0.2.4 is the target, with no version change in this planning PR. Map existing release beads to the code they already cover before filing more work. PR 249 already supplies automatic dictionary acquisition. PR 271 is pending summary work, not a missing implementation to recreate. Existing packed-install and package-manager CI jobs supply baseline coverage and need targeted assertions, not replacement. The site tab is a handoff. `provenance-tqj.6` is an open graph decision about standalone CLI bootstrap versus SDK engine delivery. `provenance-tqj.7` depends on it and covers curl and PowerShell installers. Both remain an optional, decision-gated extension and do not silently block every 0.2.4 onboarding task. Do not create an identity, migration, or release policy here.

## Verification for this draft

This planning PR contains only this document and the six graph records named above. Validate every JSONL row, ID, source reference, and parent link on main's schema version 2. Run a compatible graph check and the statement checker on all three Requirement statements. Check file and link references in this plan and compare its evidence against the pinned source and PR heads. The validation of this draft does not reproduce the historical runtime defects or test the future implementation. The later PRs must run the case-based CI gates above on their own current heads.

## Evidence appendix

The root copied the historical bead evidence before cleanup. The text below records the supplied wording and its limits. The original conversation message IDs were not supplied. These bead records locate historical reports and decisions; they do not turn research recommendations or this plan into direct user instructions. The current planning request is root thread `01a0b343-4570-72f0-b09a-390aec9b4af5`; its instruction to draft plans and necessary graph changes authorizes this PR, not every behavior proposed here.

### provenance-1azy (2026-09-12): reported pain and research recommendations

Ben's recorded pain was “no explanation of what it is” and “no signal that anything has happened.” The original research recommended a brief introduction, a truthful new or modified file inventory, quiet and non-TTY handling, an idempotent no-change summary, and numbered commands. Those recommendations were research output, not all direct user text. The numbered-command recommendation was superseded by the later amendment in `provenance-j4yq`.

### provenance-j4yq (2026-09-12 and 2026-09-13): direct request and amended closer

Ben's 2026-09-12 recorded request was: “when I run npx @quality-sh/create-provenance it should automatically install it and begin initialization.” Ben's 2026-09-13 amendment replaced the numbered-command closer with one line: “Have your agent run provenance prime to get acclimated.” The amendment keeps dictionary internals out of the successful path and calls for visible failure guidance. The bead's claim that the closer was already implemented was stale: main has no such closer, and [PR 271](https://github.com/quality-sh/provenance/pull/271) is open at `ac4f3fbbce9a01ec8675ff224d3f8b6679b814bd` with older output.

### provenance-xxb0 (2026-09-12): defaults and overrides

The original defect ticket requested omitted-option defaults `--path .` and `--scope default`, with explicit overrides preserved. These are defaults for CLI inputs. The ticket did not decide which containing project root an implicit `.` must select in a nested or mixed workspace.

### provenance-xxb0 (2026-09-12): missing manifest and partial write

The ticket reported a write that left a shard without a manifest, silent empty results from `prime` and `health`, and an error for bare `init` without `--scope`. It requested actionable init guidance and no partial state after a rejected write. These are historical reports, not a fresh reproduction on PR 317. The later coordinator reading that only graph-state-requiring commands need guidance, with `prime` exempt as general guidance, is an interpretation. It is not a recorded new human policy decision.

### Code evidence and release instruction

[PR 249](https://github.com/quality-sh/provenance/pull/249) merged at `5a422e8c261f8ceff7d42bf4d31688d933cc2ef6` and added automatic dictionary acquisition without a user-facing interactive flag. Main's `ste_onboarding.rs` binds the active acquisition and fallback Rules. The interactive-acquisition, attribution, and claim-scope Rules have deprecated status in the graph. Approved `res_ste_onboarding_auto_download_only` records the ownership notice move to `LICENSE`; PR 249 also updates `README.md`. The current user names 0.2.4 as the next release and puts onboarding after plumbing and porcelain, without a 1.0 target.
