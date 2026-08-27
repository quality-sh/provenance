# Plan: basic RBAC grants in the manifest

date: 2026-08-27
bead-id: carried by the launcher subject line ("provenance-cvs basic RBAC grants in manifest"); this worker stage is barred from `bd`, so no raw tracker id is restated here
epic: basic authorization grants for Provenance state writes
stage: plan-pending-human-review
model: glm-5.3-flash-high

## Decisions restated from review with the human, 2026-08-27

Four dated comments on the bead record these decisions. They are binding input for this plan.

1. Shape and vocabulary. The manifest gains an `rbac`-keyed section. Assignments are shaped `{actor_id, identity_type?, capabilities[], scopes[]}`. Capabilities form a closed enum of exactly `read`, `edit`, `execute`, `manifest-write`. Grants are flat and positive-only: no wildcards, no delegation, no expiry. Only standard RBAC terms appear in docs and code: principal, resource, capability, assignment. Principal identifiers are chosen to stay compatible with external auth-provider subjects, because OAuth porting is intended later.
2. Identity constraint and ratification law. An assignment that lacks the human identity type cannot hold `edit` or `execute` on lifecycle decision surfaces; alternatively an explicit marker scheme may carry that fact instead (reviewer picks one). The existing human ratification law stays as it is; fine-grained caps do not come back now.
3. Enforcement and legacy path. Enforcement rides the single aggregate-validator choke point before any write. Legacy `disposition_actor_ids` entries translate mechanically during exactly one SDK protocol-bump window and are refused when ambiguous after it. `init --disposition-actor-id` is deprecated at the same boundary.
4. Governance. Grant edits land only through Git review. In v1 the engine exposes no mutation verb for the `rbac` section. The engine makes zero authentication claims.

## APPROACH SUMMARY

Add an optional `rbac` section to `.provenance/state/manifest.json`. Each assignment names one principal, one optional identity type, positive capabilities, and explicit scopes. Four capabilities exist: `read`, `edit`, `execute`, `manifest-write`. There are no wildcards, delegation, or expiry.

Enforcement uses one shared policy function placed beside the existing aggregate validator, called before any record lands. A mapping table ties every mutating operation family known today to one capability. Unmapped verbs refuse by default. Callers claim a principal with a CLI flag or SDK field; the engine checks claims only and performs no authentication.

Legacy `disposition_actor_ids` keep working during exactly one SDK protocol window as a mechanical translation source. After the window, ambiguous manifests refuse with fixed wording, and `init --disposition-actor-id` refuses too. Core types land first. Enforcement seams land second, then legacy semantics, then tests and docs.

Grants change only through Git review. v1 adds no engine verb that writes the section.

## WORKSTREAM BREAKDOWN

### W1 — core types and wire shapes for the manifest section

Goal
Define closed Rust types for the `rbac` section, deserialize them with unknown-field refusal and snake_case wire names, keep old manifests parseable, and surface the section in `schema show`.

Touched files with citations

- `crates/provenance-core/src/model/manifest.rs:25-31` — `Manifest` today holds `schema_version`, `scopes`, and `disposition_actor_ids` with `serde(default)` at lines 29-30. Add `#[serde(default)] pub rbac: Option<RbacSection>` here. Add new types `RbacSection { assignments: Vec<Assignment> }` and `Assignment { actor_id, identity_type, capabilities, scopes }` in the same file.
- Wire closure precedent — `crates/provenance-core/src/model/ideation/contributions.rs:11` puts `#[serde(deny_unknown_fields)]` on every record struct. New RBAC structs copy that attribute. Field names already serialize snake_case by Rust naming convention; no rename attributes are needed.
- Closed read projection — `crates/provenance-store/src/state_store.rs:56-63` defines `ManifestProjection` with `deny_unknown_fields` and consumed fields only. It must learn the new key, or every manifest carrying `rbac` fails scope lookup inside `closed_manifest_scope` (`state_store.rs:103-120`). Add `#[serde(default)] rbac: Option<RbacSection>` there.
- Capability enum — a plain serde enum over `read | edit | execute | manifest-write` in `provenance-core`. Serialization matches the wire strings exactly.
- Manifest path constant — `crates/provenance-store/src/layout.rs:21-23` gives `.provenance/state/manifest.json`; nothing changes here, listed so reviewers see the file under governance.
- Schema surfacing — `crates/provenance-cli/src/handlers/schema.rs:19-36` maps `IdeationArtifactKind` variants to schema builders and wraps them in a JSON Schema envelope. Add a `Manifest` variant to `crates/provenance-cli/src/cli/ideation.rs:22-30` and an `artifacts::manifest::schema()` module beside the existing ones (`handlers/schema.rs` artifact modules at lines 7-8). Precedent: graph-reference kinds already ride this ideation-named enum.

Migration notes
Old manifests lack the key; `Option` plus `default` keeps them valid. No state schema bump: the store treats additive metadata as version 1 when optional fields are preserved (docs/state-format.md:7-18), and `Manifest` already gained a field that way (`model/manifest.rs:29-30`). Init-created empty repositories get `rbac: null` omitted on write because `init` serializes whatever `Manifest` holds (`crates/provenance-cli/src/handlers/repo.rs:87`).

Test strategy
Round-trip unit tests in `provenance-core`: parse minimal manifest with and without `rbac`; refuse unknown keys inside assignments and the section; refuse capability strings outside the four-value enum; refuse duplicate `(actor_id, scope)` pairs as one proposed closure rule. Projection test in `provenance-store`: a manifest containing `rbac` passes `closed_manifest_scope`. CLI test: `schema show --artifact manifest` emits definitions naming each capability.

Rollout gate
All new unit tests pass, and every existing manifest-parsing test still passes unchanged (for example `crates/provenance-cli/tests/cli_check.rs:151-153` golden manifest strings and `crates/provenance-cli/tests/cli_init.rs:49`).

Complexity M

### W2 — validation integration across all write seams

Goal
Make every mutation ask "does this claimed principal hold the needed capability on this resource?" before bytes move. Keep one choke function; enumerate the seams exhaustively; fall back to refusal.

The choke point, stated faithfully against the code
The bead's decision says enforcement rides the single aggregate-validator choke point. Fact: the aggregate validator itself (`crates/provenance-core/src/model/ideation/lifecycle/aggregate_validation.rs:51-115`) judges ideation records; writers outside ideation validate locally. The faithful implementation is therefore one pure policy function in `provenance-core` (beside the aggregate validator) invoked at every seam listed below, with the ideation seams reaching it through the existing aggregate call sites. Nearly all shard mutations funnel through two primitives: `StateStore::mutate_jsonl_records` (`crates/provenance-store/src/publication.rs:458-470`) and direct `mutate_jsonl_locked` callers such as verification runs (`verification_runs.rs:82`, `verification_runs.rs:130`). Gating these two primitives plus the named outliers below gives mechanical coverage. Shaping verbs use `with_lifecycle_lock` / `with_repository_publication` wrappers (`ideation_batches.rs:13-21`).

Principal claims with zero authentication
Today's precedent is attestation: a disposition actor id "records who claims to have decided", and repository write access is the only gate behind it (`aggregate_validation.rs:156-158`); docs repeat "local audit attestation, not cryptographic authentication" (docs/cli.md:396-398). V1 extends that model: every mutating request carries a claimed `actor_id`. Proposed transport: a global `--actor-id` flag on the CLI and an `actor` field on SDK mutating requests, both optional at first. Repositories without an `rbac` section behave exactly as today. Repositories with an `rbac` section refuse any mutation whose claim is missing or unauthorized, which pressures clients to upgrade without forcing every existing client through a bump at once.

Operation-family to capability mapping table
This table covers every mutating verb that exists today. Sources: handler listings under `crates/provenance-cli/src/handlers/` and writer entry points in `crates/provenance-store/src/state_store/`.

| # | Operation family | Representative writer (citation) | Proposed capability |
|---|---|---|---|
| 1 | Typed-spec apply (sources, requirements, rules, bindings, relationships reconciled) | `apply_typed_spec`, typed_specs.rs:112-120; SDK `apply`, operations.rs:79-87 and handlers/sdk.rs:38-47 | execute |
| 2 | Direct source/domain authoring | create_source writers.rs:11-55; create_domain domain_writers.rs:6 | edit |
| 3 | Requirement authoring and reference edges | create_requirement writers.rs:57-107; add_source_reference writers.rs:131-187 | edit |
| 4 | Requirement fog text | set_requirement_fog writers.rs:109-129 (docs/cli.md:435) | edit |
| 5 | Generic edge maintenance | create_edge/delete_edge writers.rs:189-230 | edit |
| 6 | Resolution/rule authoring | create_resolution rule_writers.rs:6; create_rule rule_writers.rs:89 | edit |
| 7 | Boundary/topic/question creation | create_boundary shaping_writers.rs:19-63; create_topic :65-107; create_question :109+ | edit |
| 8 | Topic/question workflow transitions | claim_topic :167; release_topic :206; close_topic :219; claim_question :227; release_question :255; answer_question :268; update_question :307 | execute |
| 9 | Thread conversation posts | post_thread_message thread_writers.rs:6 | edit |
| 10 | Ideation trio authoring (contributions, synthesis packets, proposals) | ideation_writers.rs:8-22, :127-141; proposal_writers.rs:170-203 | execute |
| 11 | Assertion recording (lifecycle evidence) | write_assertion proposal_writers.rs:135-168; assert batch path :100-115 | execute |
| 12 | Disposition creation (the lifecycle decision surface) | create_disposition/write_disposition proposal_writers.rs:205-259 | execute + identity_type=human required (decision C2) |
| 13 | Swarm landing batches | land_ideation_batch ideation_batches.rs:30-39; CLI preflight and landing handlers/swarm_backtrace.rs:66,134,144 | execute; batches containing dispositions also run family 12 checks per row |
| 14 | Verification begin/complete and binding materialization | begin_verification/:69-78 verification_runs.rs:14-114; complete_verification :116-151; materialize_verification_binding verification_bindings.rs:8; materialize_implementation_binding implementation_bindings.rs:127 | execute |
| 15 | Scope import publication | handlers/import.rs:26-99 (validation :64-72; staged apply :92) | manifest-write (it replaces whole shards); dispositions inside additionally run family 12 checks |
| 16 | Merge-driver shard commit | handlers/merge_jsonl.rs handle and `validate_merged_records` call (:16-73); merges untyped JSON, caller must re-gate (merge.rs:65-74, :76-148) | edit (shard family mapped via `ShardFamily`); disposition-family rows additionally pass family 12 identity checks |
| 17 | Project dictionary reference write | handlers/dictionary.rs import -> set_project_dictionary dictionary_reference.rs:24-39 (writes state_dir/dictionary.json :10-12) | edit |
| 18 | Repository init (manifest bootstrap and re-init) | repo.rs init :8-23; prepare_init manifest bytes :87; AGENTS/.gitignore side files :88-101 | manifest-write on re-init of an rbac-managed repo; first bootstrap exempt (see OQ7) |
| 19 | Explicit manifest edits outside init | none — no verb writes manifest.json except init (repo.rs:129-133 rolls back its own write) | manifest-write appears as capability only; the section itself has no mutation verb, per decision C4 |

Fallback policy
Any mutating seam not named above refuses by default once the previous table exists. This default-deny is the reviewer choice flagged in OQ1.

Identity law check placement
Family 12 sites pass the acting principal into the shared function; the function refuses when the assignment's identity_type is not the human value while the operation sits on a lifecycle decision surface. Wording follows the house style of fixed refusals (example precedent: `allowlist_refusal`, aggregate_validation.rs:200-207).

Touched files with citations
New module `provenance-core/src/model/rbac_policy.rs` next to the aggregate validator (`aggregate_validation.rs:51-115`); primitive hooks in `publication.rs:458-470` and `jsonl.rs` locked mutate used at verification_runs.rs:82,130; CLI flag plumbing in `cli.rs:28+` command tree and handler entry points; SDK input structs under `crates/provenance-core/src/protocol/typed_spec.rs:19-107` gain the actor claim; aggregate-fed call sites keep threading manifest data as they already do (`ideation_writers.rs:113-121`, `proposal_writers.rs:155-163,182-190,249-257`, `ideation_batches.rs:107-116`, `handlers/import.rs:64-72`, `handlers/check.rs:100`).

Migration notes
Ships dark: no behavior change until a manifest carries `rbac`. The actor claim rides the SDK protocol bump described in W3; `SDK_PROTOCOL_VERSION` is currently 5 (`crates/provenance-core/src/protocol.rs:25`) and handshakes enforce it (`protocol.rs:54-63`, advertised operations.rs:56-63).

Test strategy
One acceptance test per family row: refuse without grant, accept with matching grant, accept scoped narrowly, refuse cross-scope. Default-deny proven by simulating a new writer calling the primitives without a registered family (unit-level, since adding a real verb later inherits refusal automatically).

Rollout gate
Green suite on the full workspace plus new family tests; manual smoke of `sdk apply` against an rbac-managed fixture repo; no golden refusal string changes for repos without `rbac`.

Complexity L

### W3 — legacy translation shim and deprecation sequencing

Goal
Translate the legacy allowlist mechanically, define the one-window boundary, fix ambiguous-state refusal wording, and retire the old init flag at the same boundary.

Semantics
- Window definition: engines carrying `SDK_PROTOCOL_VERSION = 6` (`protocol.rs:25` holds 5 today) constitute the one window. Bumping to 6 ships both the actor-claim fields (W2) and translation behavior together.
- During the window: a manifest with non-empty top-level `disposition_actor_ids` and no `rbac` section behaves exactly like today (the allowlist feeds `IdeationAggregate.disposition_actor_ids`, `lifecycle.rs:70`, consumed at aggregate_validation.rs:169; empty list blocks every disposition by design, documented at aggregate_validation.rs:150-154). Each legacy id reads as `{actor_id: <id>, identity_type: human, capabilities: [the disposition authority implied by families 12], scopes: [<every scope present in the manifest>]}`. Expansion is computed at read time; nothing rewrites files during the window.
- Ambiguity rule during the window: a manifest holding both a non-empty legacy list and an `rbac` section refuses. Refusal wording (proposed): `ambiguous manifest: disposition_actor_ids and rbac.assignments are both present; move disposition actors into rbac assignments and remove disposition_actor_ids`.
- After the window (next protocol bump): drop the field from `ManifestProjection` (`state_store.rs:56-63` denies unknown fields, so stale manifests fail closed there automatically) and flip core `Manifest` (`model/manifest.rs:25-31`) to `deny_unknown_fields` when removing `disposition_actor_ids`, because core `Manifest` currently tolerates extra keys and would otherwise silently ignore the legacy list.
- Wildcard note honored: scopes expand per the scope set at translation time. Scopes added later need explicit grants; no wildcard token ever enters storage.

Init deprecation at the same boundary
`--disposition-actor-id` and `--clear-disposition-actors` live at `crates/provenance-cli/src/cli.rs:34-48` with semantics applied in `repo.rs:82-86`. During the window the flags keep working but print a warning pointing at `rbac.assignments`. After the window they refuse with wording naming the replacement section. Docs anchor points: docs/cli.md:18 (example), docs/cli.md:396-399 (attestation paragraph), plus ADR context that modern disposition actor IDs live in the manifest allowlist (docs/adr/0001-immutable-proposal-lifecycle.md:14).

Migration notes
No file migration runs; translation is read-side only. Check validation keeps passing legacy fixtures because `check` threads the same allowlist (`handlers/check/scope.rs:21-71`, `handlers/check/scope/ideation.rs:17-24`).

Test strategy
Window-inside tests: legacy-only translates (behavior equal to today, reuse scenario shapes from crates/provenance-store/src/state_store/tests/proposals/disposition_allowlist.rs:16-50 and disposition_write_gate.rs:379); rbac-only enforces; both-present refuses with the exact golden message; post-window simulation refuses the legacy key at parse time. CLI tests mirror crates/provenance-cli/tests/cli_init.rs:99-231 patterns for the deprecation warning, then refusal.

Rollout gate
Golden message tests merged; a window matrix test names all four states explicitly; release notes checklist item recorded in this doc's acceptance list.

Complexity M

### W4 — test matrix

Goal
Prove capability enforcement end to end across the enumerated families and the legacy boundary.

Matrix (each cell becomes a named test)
1. Per-capability accept/refuse: for each of `read/edit/execute/manifest-write`, one representative family from the W2 table exercised with a grant present and absent. `read` v1 gates nothing observable (reads ship ungated; see OUT OF SCOPE) — its test asserts only parsing and harmless presence, and documents that fact.
2. Cross-scope attempts: a principal granted scope A acts on scope B and refuses; B attempt on a scope-less edge family refuses identically.
3. Identity-type violation case: an assignment without the human identity type drives a disposition onto a live proposal and refuses with the fixed wording; the same call with a human-typed assignment succeeds. Reuses live-state fixtures shaped like disposition_allowlist.rs and the trigger logic provenance-core already proves (`validate_actor_allowlist`, aggregate_validation.rs:176-195 with its trigger test at :377-413).
4. Legacy-compat window, both sides: covered by W3 matrix items, executed through real CLI invocations using the harness pattern in crates/provenance-cli/tests/cli_import_lifecycle/support.rs:12-22 where actor ids seed init.
5. Swarm landing composition: a batch mixing contributions plus dispositions passes/failed correctly per-row (family 13 composition).
6. Merge-driver gate: a merged dispositions shard bypassing direct writes still refuses when the survivor lacks authority, asserting the merge.rs warning scenario (`merge.rs:65-74`).
7. Import round trip: export of an rbac repo re-imports (export preserves canonical records; graph-reference exclusion list confirms manifest handled separately, docs/state-format.md:115-120).
8. Unmapped fallback: default-deny proof per W2.
9. Schema surface: `schema show --artifact manifest` matches its compiled expectation.

Touched files with citations
Test homes follow existing layout: store-level suites under crates/provenance-store/src/state_store/tests/proposals/ (see lifecycle_validation.rs:449 for allowlist seeding style); crate-level suites in crates/provenance-core/src/model/tests/proposal_lifecycle_dispositions.rs:27-284 (refusal message goldens exist at :260-261); CLI black-box suites under crates/provenance-cli/tests/ (cli_init.rs, cli_check.rs:151-153 manifest goldens, cli_import_legacy_audit.rs:263-265).

Rollout gate
Whole matrix green locally and in CI; count of new tests recorded in the PR description.

Complexity M

### W5 — docs updates inventory

Goal
Keep user-facing truth aligned with shipped behavior and list the architecture decisions worth recording.

Inventory
1. docs/state-format.md — new paragraph after the manifest sentence at line 5 documenting the `rbac` section shape, flat positive-only grants, Git-review-only editing, and the additive-no-version-bump stance mirroring lines 7-18.
2. docs/cli.md — update the init example at line 18; extend the attestation paragraph at lines 396-399 with the claimed-principal model and the `--actor-id` flag; add one sentence near the schema coverage at lines 380-389 noting `schema show --artifact manifest`.
3. ADR candidates list (new numbered file drafts deferred to implementation):
   - ADR 0009 candidate: basic RBAC grants in the manifest. Mandatory content: governance privilege-escalation statement — grants are flat and positive-only, carry no wildcards, delegation, or expiry, change only through reviewed commits, and the engine exposes no verb that writes the section in v1; ratification law untouched per docs/adr/0001-immutable-proposal-lifecycle.md (its allowlist statement at line 14 predates and remains true during the window).
   - ADR 0010 candidate (conditional): chosen identity-constraint encoding (enum vs explicit marker scheme, OQ2).
   - ADR 0011 candidate (optional): unmapped-operation fallback policy (OQ1 outcome).
4. CONTEXT.md domain terms — add principal, resource, capability, assignment only (decision C4 restricts vocabulary); editing CONTEXT.md is part of the implementation bead, listed here so reviewers see the touchpoint.

Test strategy
Docs-only diff; verify examples compile conceptually by pasting commands from the test suite in W3/W4 (flag strings must match the implemented CLI exactly).

Rollout gate
Reviewer finds no doc contradicting shipped behavior; grep for `wildcard`, `delegat`, `expir` in new sections returns no normative grant features.

Complexity S

## OPEN QUESTIONS FOR HUMAN REVIEW

1. Fallback policy confirmation. Recommendation: unmapped mutating verbs refuse by default. Alternative: log-and-allow during the transition. Flagged as reviewer choice per the bead summary.
2. Identity-constraint encoding. Option A: `identity_type` closed enum `{"human","service"}`, absent field defaults to `"human"` (preserves today's attestation behavior). Option B: unconstrained type plus explicit marker scheme on decision surfaces. Decision C2 allows either; pick one.
3. Lifecycle decision surface enumeration. Proposal: disposition creation including dispositions arriving inside swarm batches, imports, and merged shards. Excluded: assertions, proposal creation, topic closes, question answers. Confirm the boundary.
4. Family split sanity. Two rows deserve scrutiny: typed-spec apply as `execute` even though it retires/updates requirement content (row 1); topic/question creation as `edit` versus their transitions as `execute` (rows 7 vs 8). Confirm or redraw.
5. Claimed-principal transport. Global `--actor-id` plus SDK `actor` field, activated by bumping `SDK_PROTOCOL_VERSION` from 5 (protocol.rs:25) — confirm this bump is the intended "one protocol-bump window".
6. Translation expansion risk. Expanding a repo-global legacy allowlist to every current scope grows stale after new scopes appear; new scopes then need fresh grants. Acceptable, or prefer dual authority until window close?
7. Bootstrap. First init on an empty repository cannot consult the section it creates; re-init of an rbac-managed repo demands `manifest-write`. Confirm the bootstrap exemption.
8. State schema version stays 1. Additive optional field, precedent docs/state-format.md:7-18. Confirm no version bump accompanies the section.

## ACCEPTANCE CHECKLIST

| Promised outcome | Observable verification |
|---|---|
| Manifest accepts `rbac` section; unknown keys inside it refuse | `cargo test -p provenance-core` includes new round-trip and deny tests for `model/manifest.rs` types |
| Old manifests keep parsing everywhere | Existing suites pass unmodified: `crates/provenance-cli/tests/cli_check.rs:151-153`, `cli_init.rs`, `state_store/tests/proposals/*` |
| Closed read projection understands the key | `provenance-store` test exercising `closed_manifest_scope` (state_store.rs:103-120) with an `rbac`-bearing manifest |
| Every mutating family maps to a capability; all others refuse | W2 family-by-family store tests; fallback test refusing an unregistered synthetic writer |
| Claims without authority refuse cross-scope | Matrix test 2 in W4 |
| Non-human identity cannot drive lifecycle decisions | Matrix test 3 golden refusal message, reuse of aggregate-validation fixture style (aggregate_validation.rs:384-413) |
| Legacy allowlist translates inside the window | W3 four-state matrix (legacy-only, rbac-only, both-refused golden, post-window refusal), building on disposition_allowlist.rs:16-50 scenarios |
| `init --disposition-actor-id` warns during window, refuses after | CLI tests cloned from cli_init.rs:99-231 with warning and refusal assertions |
| No engine mutation verb writes the section in v1 | Absence assertion: CLI `--help` snapshot lists no manifest-mutation subcommand; public API grep gate documented in PR |
| `schema show` surfaces the section | `provenance schema show --artifact manifest` emits JSON Schema naming `rbac`, four capabilities; CLI test beside handlers/schema.rs tests module (:50-51) |
| Docs match behavior | Sections exist at docs/state-format.md line 5 area and docs/cli.md lines 18, 380-399; ste100 dictionary does not govern docs, so reviewer eyeball plus link check |
| ADR candidates recorded with mandatory escalation statement | Draft bullets visible in W5; final ADRs land in their own implementation bead |

## OUT OF SCOPE RESTATED

- SQLite served-read flip (`provenance-1wh`) is adjacent work and stays out of this plan. Interfaces where read-gating may later attach without redesign: the reader layer (`StateStore::list_*`, `crates/provenance-store/src/state_store.rs:146-250`), the closed readers module (`readers.rs`, wired at `state_store.rs:50-53`), the projection helpers (`project_proposal_cards`, `state_store.rs:290-318`), and the query/response envelope built in `crates/provenance-core/src/protocol/response.rs`.
- Authentication, signatures, tokens, identity proofs: excluded forever by decision C4 for v1; the actor id remains an attestation (precedent citation aggregate_validation.rs:156-158).
- Delegation, expiry, wildcard scopes, revocation tooling, role hierarchies: excluded by the flat positive-only decision (C1).
- Read enforcement of `read`: planned attach points noted above, activation out of scope.
- Any external auth-provider/OAuth integration beyond choosing subject-compatible identifier syntax (C1).
- Migration scripts rewriting manifests: translation stays read-side during the window (W3).
- Wiki ownership marker and cache artifacts: different files entirely (`crates/provenance-cli/src/wiki/publish/manifest.rs:9-14` describes an OwnershipManifest generator marker; lock and cache files are non-state per docs/state-format.md:99-105); untouched by this plan.

## FACTS AND INFERENCES

Facts (code checked this session)

- `Manifest` holds exactly `schema_version`, `scopes`, `disposition_actor_ids` with `serde(default)` (crates/provenance-core/src/model/manifest.rs:25-31).
- Store reads go through `StateStore::manifest()` (state_store.rs:94-101) and a `deny_unknown_fields` projection (state_store.rs:56-63) used by scope lookups (:103-120).
- The ideation aggregate validator is the central ideation gate (aggregate_validation.rs:51-115); the disposition actor allowlist rule, its refusal copies, its "empty list blocks everything" posture, and the attestation caveat live at aggregate_validation.rs:150-207.
- Writers exist for sources, requirements, fog, references, edges (writers.rs:11-230), domains (domain_writers.rs:6), resolutions/rules (rule_writers.rs:6,89), threads (thread_writers.rs:6), boundaries/topics/questions plus transitions (shaping_writers.rs:19-307), ideation trio and dispositions (ideation_writers.rs:8-141, proposal_writers.rs:100-259), batch landings (ideation_batches.rs:30-39,116), typed specs (typed_specs.rs:112-131), verification runs and bindings (verification_runs.rs:14-151, verification_bindings.rs:8, implementation_bindings.rs:127), dictionary reference (dictionary_reference.rs:24-39 writing state_dir/dictionary.json at :10-12).
- Import validates then applies staged state under the publication lock (handlers/import.rs:61-93); merge-driver output is re-gated through shard-typed validation (handlers/merge_jsonl.rs:16-73; merge.rs:65-74).
- Init is the only manifest writer (repo.rs:87, rollback journal :127-161); its flags sit at cli.rs:34-48.
- `SDK_PROTOCOL_VERSION` is 5 with handshake refusal elsewhere (protocol.rs:25,54-63) and advertisement through EngineInfo (operations.rs:56-63).
- Schema show derives from `IdeationArtifactKind` (handlers/schema.rs:19-36; cli/ideation.rs:22-30).
- The wiki ownership manifest is unrelated (wiki/publish/manifest.rs:9-14). Cache/lock files are declared non-state (docs/state-format.md:70-73,99-105).

Inferences (judgment, pending reviewer disposal via the open questions)

- "Single aggregate-validator choke point" is best realized as one pure policy function beside the aggregate validator, invoked from each enumerated seam, because non-ideation writers never enter aggregate validation today (files cited above).
- The "one SDK protocol-bump window" maps naturally onto raising `SDK_PROTOCOL_VERSION` 5→6 alongside the actor-claim fields; nothing in the inputs pinned the number, only the mechanism.
- Post-window refusal of the legacy key is mechanical once `ManifestProjection` drops the field; core `Manifest` additionally needs `deny_unknown_fields` or it would ignore stale keys silently (core struct lacks the attribute today, manifest.rs:25-31).
- Operation-to-capability assignments, the family boundaries, default-deny, bootstrap exemption, and identifier syntax details are proposals, not recorded decisions; open questions 1-8 hold them for disposal.
