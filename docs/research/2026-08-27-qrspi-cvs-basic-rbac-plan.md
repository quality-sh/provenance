# Plan: basic RBAC grants in the manifest (revised after maintainability review)

date: 2026-08-27
revised: 2026-08-28 — applies the independent maintainability review, the binding human disposal, and the strict-audit corrections (merge-driver claim transport; repo-global scope blocking question)
bead: provenance-cvs
epic: provenance-46p
stage: plan-pending-human-review
model: glm-5.3-flash-high

No implementation begins from this session. This document only specifies HOW.

## What this revision changed

The prior revision of this plan (commit `0ad80e7`) was reviewed independently. This revision:

1. Replaces the "mechanical translation" legacy story. Legacy-only manifests keep the existing
   `disposition_actor_ids` rule byte-for-byte during the window. No synthesized RBAC assignments.
   No expansion of disposition-only authority into `execute`.
2. Adds the post-window ratification migration that retires `IdeationAggregate.disposition_actor_ids`
   and `validate_actor_allowlists` in the same change that removes the manifest field, with every
   feeder and public validation API enumerated.
3. Defines one explicit typed authorization context (`RbacClaim`) supplied top-down, resolved inside
   the publication lock. No ambient store field, thread-local, claim sniffing, or scattered refusals.
   The census now names `requirement_reviews` and `typed_specs` explicitly, and import gets a named
   explicit gate.
4. Closes the merge bypass: unrecognized shard families and shard-path-less merge output refuse in
   RBAC-managed repositories, and disposition rows in merged output pass the decision-surface checks.
5. Reuses the existing `IdentityType { human, agent, service }` enum. No second enum. Missing
   `identity_type` fails closed for human-ratification operations. Removes the settled
   marker-scheme and protocol-window questions.
6. Names one ambiguity-refusal function called by every manifest reader, with golden tests.
7. Adds the mandated test list (re-init preserves RBAC bytes; missing claim vs wrong principal;
   execute plus human identity; legacy-only unchanged; both regimes refuse; post-window non-deadlock;
   import and merge cannot bypass).
8. Adds responsibility-based file extraction gates before any change presses the hard 500-line limit.
9. Corrects metadata: bead `provenance-cvs`, epic `provenance-46p`.
10. Makes the merge-driver claim transport mechanically real. Fact: `.gitattributes` only selects
   `merge=provenance-jsonl` (`.gitattributes:3`); it passes no arguments. The command template is
   clone-local git config (`docs/cli.md:281-291`). Automatic Git invocation gets the actor as a
   literal `--actor-id <id>` argument in that configured command; the handler still receives a
   typed `RbacClaim` and refuses no-claim, unauthorized, unrecognized-family, path-less, and
   bad-disposition-identity merges (W4).
11. Stops hiding the repo-global scope policy. The every-scope rule becomes blocking question 9
   with named alternatives; census rows 17, 19, and 20 cannot be implemented until the human
   disposes it (W2, OPEN QUESTIONS).

## Decisions settled by the human, 2026-08-27 (binding)

These are normative. This plan does not reopen them.

- D1 — Shape and vocabulary. The manifest gains an `rbac`-keyed section with flat, positive-only
  assignments shaped `{actor_id, identity_type?, capabilities[], scopes[]}`. The capability set is
  closed at exactly `read`, `edit`, `execute`, `manifest-write`. No wildcard, no delegation, no
  expiry, no fine-grained lifecycle capabilities. Standard RBAC terms only: principal, resource,
  capability, assignment. Principal identifiers are chosen to stay compatible with external
  auth-provider subjects; the engine makes no authentication claim.
- D2 — Identity and ratification. `identity_type` reuses the existing core enum
  `IdentityType { human, agent, service }` (`crates/provenance-core/src/model/ideation.rs:74-93`).
  No second enum is created. An assignment that omits `identity_type` fails closed for
  human-ratification operations. Human ratification is preserved through `identity_type` validation.
- D3 — Legacy window. Legacy `disposition_actor_ids` survives for exactly one SDK protocol-bump
  window. Inside the window, legacy-only manifests keep the existing rule byte-for-byte. Both
  regimes present is ambiguous and refuses. `init --disposition-actor-id` deprecates at that
  boundary.
- D4 — Post-window migration. In the same change that removes `disposition_actor_ids`, the
  aggregate allowlist law is replaced by the RBAC identity rule, and every feeder and public
  validation API is migrated so the old empty-allowlist law cannot deadlock valid dispositions.
- D5 — Enforcement. One shared policy choke runs before every write. Authorization arrives as one
  explicit typed claim supplied top-down from CLI/SDK into mutation entry points, and is resolved
  against the manifest inside the publication lock. Import has a named explicit gate because it
  writes outside the normal primitives.
- D6 — Governance. Grant edits land only by Git review. v1 exposes no engine verb that writes the
  `rbac` section.

## Hard repository constraint: the 500-line file limit

Every Rust file in this repository, tests included, has a hard 500-line limit. Measured this session:

| File | Lines now | Mandated extraction before RBAC work touches it |
|---|---|---|
| `crates/provenance-store/src/publication.rs` | 476 | Extract access/snapshot helpers (`snapshot_state` :400-412, `copy_tree` :414-431, `with_state_path_access` :433-448, `sync_directory`/`sync_tree` :367-398) into a sibling module before adding the gate hook. |
| `crates/provenance-store/src/state_store/ideation_batches.rs` | 491 | Extract merge helpers (`merge_replaceable` :386-408, `merge_immutable` :410-430, `insert_all` :344-359, `ensure_scope` :361-384, `overlay_records` :479-491) into a sibling module before adding batch gating. |
| `crates/provenance-store/src/state_store/shaping_writers.rs` | 468 | Split topic/question transitions (`claim_topic` :167 through `update_question` :359) into a sibling module if this file is touched. |
| `crates/provenance-cli/src/handlers/import.rs` | 437 | Keep exactly one delegated gate call at the entry. If the file must grow, extract staging/publication helpers (`apply_import` :207-286, `rollback_publication` :343-359) first. |

Rules: extraction is responsibility-based, not mechanical line-shuffling; new RBAC types, the policy
choke, and their tests live in their own modules (W1); any proposed change that would push a file
over, or within roughly 25 lines of, the limit extracts first. `provenance-core` tests already live
in separate files (example: `crates/provenance-core/src/model/tests/proposal_lifecycle_dispositions.rs`),
and new suites follow that layout.

## APPROACH SUMMARY

Add an optional `rbac` section to `.provenance/state/manifest.json`
(`crates/provenance-store/src/layout.rs:21-23`). Each assignment names one principal, one optional
identity type, positive capabilities, and explicit scopes. Four capabilities exist. There are no
wildcards, delegation, or expiry.

Every mutating request carries an explicit `RbacClaim { actor_id }` from the CLI or SDK down into
the mutation entry point. One pure policy function in `provenance-core` resolves the claim against
the `rbac` section inside the publication lock and refuses anything unauthorized. A census table
maps every mutating operation family known today to one capability. A verb outside the census has no
mapped capability, so the choke refuses it; default-deny is a mechanical consequence of the closed
census, not a new policy choice.

Legacy `disposition_actor_ids` keeps its exact current meaning during one SDK protocol-bump window.
Repos without an `rbac` section behave exactly as they do today. A manifest holding both a non-empty
legacy list and an `rbac` section refuses. At the next protocol bump the legacy field, the aggregate
allowlist law, and the init flags are removed together and replaced by the RBAC identity rule.

Grants change only through Git review. v1 adds no engine verb that writes the section.

## WORKSTREAM BREAKDOWN

### W1 — core types, read law, and the one ambiguity refusal

Goal
Define closed Rust types for the `rbac` section, deserialize them with unknown-field refusal, keep
old manifests parseable, define the read-side ambiguity law as one named function, and surface the
section in `schema show`.

Types (new module `crates/provenance-core/src/model/rbac/`)
- `types.rs`: `RbacSection { assignments: Vec<Assignment> }`, `Assignment { actor_id, identity_type:
  Option<IdentityType>, capabilities: Vec<Capability>, scopes: Vec<String> }`, and
  `Capability` over `read | edit | execute | manifest-write`. `identity_type` is the existing
  `IdentityType` (`model/ideation.rs:74-93`); its serde wire values are already `human`, `agent`,
  `service`. `scopes` hold manifest scope IDs. `actor_id` is a non-empty string; no engine-side
  parsing beyond non-emptiness, so external subject IDs pass through unchanged (D1).
- `policy.rs`: the shared choke (W2) and the ambiguity refusal (below).
- `tests.rs` or `tests/`: round-trip and refusal goldens, kept inside the 500-line limit.
- Wire closure precedent: `#[serde(deny_unknown_fields)]` on every new struct, as
  `crates/provenance-core/src/model/ideation/contributions.rs:10` does.

Manifest integration
- `crates/provenance-core/src/model/manifest.rs:25-31` gains
  `#[serde(default, skip_serializing_if = "Option::is_none")] pub rbac: Option<RbacSection>`.
  `skip_serializing_if` keeps `init` output byte-stable for repos without the section, because
  `init` serializes whatever `Manifest` holds (`crates/provenance-cli/src/handlers/repo.rs:87`) and
  `Manifest::default_with_scope` builds the fresh manifest (`model/manifest.rs:33-43`; golden at
  `crates/provenance-cli/tests/cli_init.rs:49`).
- Closed read projection: `ManifestProjection` (`crates/provenance-store/src/state_store.rs:56-63`,
  `deny_unknown_fields`, consumed by `closed_manifest_scope` at :103-120 through
  `deserialize_closed`, `crates/provenance-store/src/state_store/readers.rs:259`) must learn the new
  key or every `rbac`-bearing manifest fails scope lookup. It gains
  `#[serde(default)] rbac: Option<RbacSection>`.
- `Manifest` itself stays tolerant of unknown keys in W1. Adding `deny_unknown_fields` to core
  `Manifest` happens only in the W3 removal change, so legacy fixtures are never refused early.

One ambiguity-refusal function (mandated)
- Name: `ensure_unambiguous_rbac` in `crates/provenance-core/src/model/rbac/policy.rs`.
- Law: refuse when the manifest holds a non-empty top-level `disposition_actor_ids` and an `rbac`
  section at the same time. An empty legacy array next to `rbac` is unambiguous — this matters
  because `Manifest::default_with_scope` writes `"disposition_actor_ids": []`
  (`model/manifest.rs:41`, `crates/provenance-core/src/scope.rs:78`) and every fresh init ships it.
- Callers, all of them: `StateStore::manifest` (`state_store.rs:94-101`), the closed projection
  path in `closed_manifest_scope` (`state_store.rs:103-120`), and repository parsing in
  `crates/provenance-cli/src/handlers/repo.rs:186-190`. Any future manifest reader must call it;
  this list is part of the acceptance checklist.
- Golden refusal wording (proposed, fixed):
  `ambiguous manifest: disposition_actor_ids and rbac.assignments are both present; move disposition actors into rbac assignments and remove disposition_actor_ids`.

Schema surfacing
- `crates/provenance-cli/src/handlers/schema.rs:19-36` maps `IdeationArtifactKind` variants to
  schema builders. Add a `Manifest` variant to `crates/provenance-cli/src/cli/ideation.rs:21-30`
  and an `artifacts::manifest::schema()` module beside the existing ones
  (`handlers/schema.rs:7-8`). Precedent: graph-reference kinds already ride this enum.

Test strategy
Round-trip tests in `provenance-core`: parse with and without `rbac`; refuse unknown keys inside
assignments and the section; refuse capability strings outside the four-value enum; refuse duplicate
`(actor_id, scope)` pairs inside one section. Golden tests for `ensure_unambiguous_rbac`: both
refuses, empty-legacy-plus-rbac passes, legacy-only passes. Projection test in `provenance-store`:
an `rbac`-bearing manifest passes `closed_manifest_scope`. CLI test: `schema show --artifact
manifest` emits definitions naming each capability.

Rollout gate
New unit tests pass and every existing manifest-parsing test passes unchanged (for example
`crates/provenance-cli/tests/cli_check.rs:151-153` golden manifest strings).

Complexity M

### W2 — explicit claims, one policy choke, and the full mutation census

Goal
Make every mutation ask "does this claimed principal hold the needed capability on this resource?"
before bytes move, through one choke function fed by one explicit claim.

The claim, top-down (mandated shape)
- One type: `RbacClaim { actor_id: String }` in `crates/provenance-core/src/model/rbac/types.rs`.
- Transport: a global `--actor-id` CLI flag (clap `global = true`; precedent `quiet` at
  `crates/provenance-cli/src/cli.rs:21-22`) read once and threaded through dispatch
  (`crates/provenance-cli/src/handlers/mod.rs:49-253`) into every mutating handler, and an `actor`
  claim field on SDK mutating requests. SDK input structs deny unknown fields today
  (`crates/provenance-core/src/protocol/typed_spec.rs:18-32`), so the claim rides the protocol bump
  that opens the window (W3); store-side inputs are re-exported at
  `crates/provenance-store/src/state_store.rs:26-37`.
- Resolution: mutating entry points pass the claim to the policy function, which reads the manifest
  inside the publication lock. The primitives already enter that lock
  (`crates/provenance-store/src/publication.rs:46-65`, `:458-470`), so the check runs against
  manifest bytes no writer can move concurrently.
- What is forbidden: no ambient field on `StateStore`, no thread-local claim, no deriving the
  principal from paths or environment, no refusal logic outside `policy.rs`. Note the boundary:
  classifying the resource by shard path is required and allowed (it answers "which resource?");
  discovering the principal from anything other than the explicit claim is not.

The choke point
- One pure function `authorize(claim, section, needed: Capability, resource) -> Result<()>` in
  `crates/provenance-core/src/model/rbac/policy.rs`, placed beside the aggregate validator it
  extends (`crates/provenance-core/src/model/ideation/lifecycle/aggregate_validation.rs:51-115`).
  Fact: the aggregate validator judges ideation records only; writers outside ideation validate
  locally, so a single store-level choke is the faithful realization of "one shared policy choke".
- Two mechanical backstops inside `provenance-store` give the census teeth:
  `StateStore::mutate_jsonl_records` (`publication.rs:458-470`) and the direct
  `mutate_jsonl_locked` callers (`crates/provenance-store/src/jsonl.rs`; used at
  `crates/provenance-store/src/state_store/verification_runs.rs:82` and `:130`) require the claim
  argument and refuse an unauthorized or absent claim on an `rbac`-managed repository. Shaping
  verbs ride `with_lifecycle_lock` (`ideation_batches.rs:13-21`) and the publication wrapper, both
  of which the backstop sits under.
- Repositories without an `rbac` section behave exactly as today. Repositories with one refuse any
  mutation whose claim is missing or unauthorized (W5 tests cover missing-vs-wrong wording).

Operation-family to capability census (every mutating verb today; unmapped verbs refuse)

| # | Operation family | Writer (citation) | Capability |
|---|---|---|---|
| 1 | Typed-spec apply (typed_specs) | `apply_typed_spec` `crates/provenance-store/src/state_store/typed_specs.rs:112-120`; writes at :217-233 through `mutate_jsonl_records` (:396-405); SDK `apply` `crates/provenance-store/src/operations.rs:79-87`, `crates/provenance-cli/src/handlers/sdk.rs:38-47` | execute |
| 2 | Direct source/domain authoring | `create_source` writers.rs:11; `create_domain` `crates/provenance-store/src/state_store/domain_writers.rs:6` | edit |
| 3 | Requirement authoring and reference edges | `create_requirement` writers.rs:57; `add_source_reference` writers.rs:131 | edit |
| 4 | Requirement fog text | `set_requirement_fog` writers.rs:111 | edit |
| 5 | Generic edge maintenance | `create_edge` writers.rs:189; `delete_edge` writers.rs:221 | edit |
| 6 | Resolution/rule authoring | `create_resolution` `crates/provenance-store/src/state_store/rule_writers.rs:6`; `create_rule` :89 | edit |
| 7 | Boundary/topic/question creation | `create_boundary` `crates/provenance-store/src/state_store/shaping_writers.rs:19`; `create_topic` :65; `create_question` :109 | edit |
| 8 | Topic/question workflow transitions | `claim_topic` shaping_writers.rs:167; `release_topic` :206; `close_topic` :219; `claim_question` :227; `release_question` :255; `answer_question` :268; `update_question` :307 | execute |
| 9 | Thread conversation posts | `post_thread_message` `crates/provenance-store/src/state_store/thread_writers.rs:6` | edit |
| 10 | Ideation trio authoring | `create_contribution`/`upsert_contribution` `crates/provenance-store/src/state_store/ideation_writers.rs:8,16`; synthesis :127,135; `create_proposal_card` `crates/provenance-store/src/state_store/proposal_writers.rs:170` | execute |
| 11 | Assertion recording | `write_assertion` proposal_writers.rs:135-168; assert batch path :100-115 | execute |
| 12 | Disposition creation (lifecycle decision surface) | `create_disposition`/`write_disposition` proposal_writers.rs:205-271 | execute + recorded actor must resolve to an assignment with `identity_type: human` (D2); missing `identity_type` refuses |
| 13 | Swarm batch landing | `land_ideation_batch` `crates/provenance-store/src/state_store/ideation_batches.rs:30-39`; CLI preflight and landing `crates/provenance-cli/src/handlers/swarm_backtrace.rs:134,144` | execute; dispositions inside a batch run family-12 checks per row at the store seam. Fact: the CLI refuses dispositions in swarm merge output today (`swarm_backtrace.rs:77-79`), but `land_ideation_batch` is a public store API that accepts them (`state_store.rs:70-82`), so the per-row check belongs in `write_ideation_batch` |
| 14 | Verification begin/complete and binding materialization | `begin_verification` verification_runs.rs:14-114; `complete_verification` :116-151; `materialize_verification_binding` `crates/provenance-store/src/state_store/verification_bindings.rs:8`; `materialize_implementation_binding` `crates/provenance-store/src/state_store/implementation_bindings.rs:127` | execute |
| 15 | Requirement review recording (requirement_reviews) | `record_requirement_reviews` `crates/provenance-store/src/state_store/requirement_reviews.rs:62-84`, called only from `apply_typed_spec` (`typed_specs.rs:232`) | inherits family 1 (execute); the write itself rides `mutate_jsonl_records` (:75) under the backstop |
| 16 | Requirement review clearing (requirement_reviews) | `clear_requirement_reviews` requirement_reviews.rs:90-112, called from `begin_verification` (`verification_runs.rs:112`) | inherits family 14 (execute) |
| 17 | Scope import publication | `import_scope` `crates/provenance-cli/src/handlers/import.rs:26-99` (aggregate feed :64-72; staged apply :92) | manifest-write via the named explicit import gate (W4); dispositions inside run family-12 checks |
| 18 | Merge-driver shard commit | `handle` `crates/provenance-cli/src/handlers/merge_jsonl.rs:20-72`; `validate_merged_records` call :46-55 | edit by shard family; refused families and disposition rows per W4 |
| 19 | Project dictionary reference write | `handlers/dictionary.rs:17-41` -> `set_project_dictionary` `crates/provenance-store/src/dictionary_reference.rs:24-39`, direct `std::fs::write` of `state_dir/dictionary.json` (:31-37) | edit; named outlier gate because the write bypasses both primitives |
| 20 | Repository re-init of an `rbac`-managed repo | `init`/`prepare_init` `crates/provenance-cli/src/handlers/repo.rs:8-23,39-114` | manifest-write. First bootstrap on an empty repository is exempt: the section it creates cannot be consulted before it exists. Re-init demands `manifest-write` and must preserve `rbac` bytes when its flags omit it (W5 test 1) |
| 21 | Manifest section edits outside init | none — init is the only manifest writer (`repo.rs:87`, rollback write :129-133) | no capability; v1 has no verb for the section (D6) |

Scope reading for repo-global resources (BLOCKING QUESTION 9 — not implementation's to settle)
Facts: the manifest itself and `dictionary.json` sit outside any one scope; they affect every
scope then listed (writers: `repo.rs:129-133` manifest write, `dictionary_reference.rs:31-37`
direct write, import's staged swap `handlers/import.rs:207-286`). D1 fixes the four capabilities
and positive-only explicit scopes; it does not say how a capability on scopes relates to a
resource that spans them. This plan's proposal — the capability must be held on every scope then
in the manifest — is the fail-safe option of question 9 below. The human names one option before
any repo-global gating is implemented. Positive-only; no wildcard token exists.

Fixed refusal wordings (proposed, goldens in W5)
- Missing claim: `rbac: no actor claim supplied for a mutating operation on an rbac-managed repository`.
- Wrong principal: `rbac: actor <id> does not hold capability <cap> on scope <scope>`.
- Ratification failure (family 12): `rbac: disposition actor <id> needs an assignment with identity_type human to end a live proposal`. The same wording covers an assignment whose `identity_type` is absent (D2 fail-closed).

Touched files with citations
New `model/rbac/` modules (W1); primitive hooks in `publication.rs:458-470` and `jsonl.rs`
(`mutate_jsonl_locked`); store method signatures gain the claim parameter (writers listed in the
census); CLI flag and dispatch (`cli.rs:28+`, `handlers/mod.rs:49-253`); SDK inputs
(`protocol/typed_spec.rs:18-32`, `state_store.rs:26-37`). Aggregate-fed call sites already thread
manifest data and keep doing so: `ideation_writers.rs:115,225`, `proposal_writers.rs:157,184,251`,
`ideation_batches.rs:107-116`, `handlers/import.rs:64-72`, `handlers/check.rs:100`.

Migration notes
Ships dark: no behavior change until a manifest carries `rbac`. The claim fields ride
`SDK_PROTOCOL_VERSION` 6 (W3); the constant is 5 today
(`crates/provenance-core/src/protocol.rs:25`, handshake :54-63, advertisement
`operations.rs:56-63`).

Test strategy
One acceptance test per census row: refuse without grant, accept with matching grant, accept scoped
narrowly, refuse cross-scope. Default-deny proven by simulating a new writer calling the primitives
without a registered family. Missing-claim and wrong-principal produce the two distinct goldens.

Rollout gate
Green workspace suite plus census tests; manual smoke of `sdk apply` against an `rbac` fixture; no
golden refusal string changes for repos without `rbac`. Hold: implementation of the repo-global
rows (17 import, 19 dictionary, 20 re-init) and the W4 import-gate capability check does not begin
until the human disposes question 9 by naming one option. Per-scope rows (1-16, 18) are not
blocked.

Complexity L

### W3 — legacy window and the post-window ratification migration

Goal
Keep the legacy law byte-for-byte inside one protocol window, refuse the ambiguous mix, then remove
the legacy field, its aggregate law, and its init flags in one change, replacing them with the RBAC
identity rule.

Window definition
- The window opens when `SDK_PROTOCOL_VERSION` moves 5 -> 6 (`crates/provenance-core/src/protocol.rs:25`),
  shipping the claim fields (W2) and the ambiguity refusal (W1) together. The window closes at the
  next protocol bump, in the same change that removes `disposition_actor_ids`. The mechanism
  (protocol bump) is settled; the specific numbers are inference from the current constant.

Inside the window
- Legacy-only (non-empty `disposition_actor_ids`, no `rbac`): the existing rule applies
  byte-for-byte. The allowlist feeds `IdeationAggregate.disposition_actor_ids`
  (`crates/provenance-core/src/model/ideation/lifecycle.rs:70`) and is consumed by
  `validate_actor_allowlists` (`aggregate_validation.rs:159-174`), with the empty-list-blocks-all
  posture (`aggregate_validation.rs:150-154`) and the attestation caveat (:156-158) unchanged. The
  refusal strings from `allowlist_refusal` (`aggregate_validation.rs:200-207`) do not change. No
  translation happens; nothing rewrites files; legacy authority stays disposition-only and is never
  expanded into `execute` or any other capability (D3).
- `rbac`-only: full W2 enforcement. The legacy law has nothing to say; the manifest carries no
  non-empty legacy list.
- Both (non-empty legacy list and `rbac` section): `ensure_unambiguous_rbac` refuses with the W1
  golden.
- `init --disposition-actor-id` / `--clear-disposition-actors` (`crates/provenance-cli/src/cli.rs:43-47`,
  semantics at `crates/provenance-cli/src/handlers/repo.rs:82-86`) keep working and print a
  deprecation warning pointing at `rbac.assignments`. Docs anchors: `docs/cli.md:18` (example),
  `docs/cli.md:395-399` (attestation paragraph), `docs/adr/0001-immutable-proposal-lifecycle.md:14`.

Post-window migration (one change, mandated)
- Wire removal. Drop `disposition_actor_ids` from core `Manifest`
  (`model/manifest.rs:25-31`) and add `deny_unknown_fields` to it, so a stale manifest refuses at
  parse with an error naming the field; drop the field from `ManifestProjection`
  (`state_store.rs:56-63`), which already denies unknown fields and so fails closed automatically.
  The refusal is serde's unknown-field error naming `disposition_actor_ids`; the golden test asserts
  that name appears.
- Aggregate law replacement. Retire `validate_actor_allowlists` and `validate_actor_allowlist`
  (`aggregate_validation.rs:159-195`) and `allowlist_refusal` (:200-207). The replacement
  ratification rule keeps the trigger reading — a proposal whose effective pre-disposition state is
  live (`aggregate_validation.rs:183-186`) — and asks instead: the disposition's recorded actor id
  resolves to an `rbac` assignment whose `identity_type` is `Human`; an assignment without
  `identity_type` refuses (D2). The `RbacClaim` of the mutating principal separately needs family-12
  `execute` (W2). Two checks, both in `policy.rs`.
- Feeder and public-API census (every site this session's search found; all must migrate so the old
  empty-allowlist law cannot survive as a hidden second gate):
  - Field: `IdeationAggregate.disposition_actor_ids` (`lifecycle.rs:70`) — replaced by the resolved
    ratification input (the repo's assignments or a resolved rule value).
  - Consumption: `aggregate_validation.rs:169`.
  - Store feeders: `ideation_batches.rs:110` (`write_ideation_batch`), `:129`
    (`validate_ideation_scope`), `:133-141` (`validate_ideation_scope_with_actor_ids`),
    `:223` (`validate_ideation_scope_snapshot`); `ideation_writers.rs:115,225`;
    `proposal_writers.rs:157,184,251`.
  - Public validation APIs to retire or re-key: `StateStore::validate_ideation_scope_with_actor_ids`
    (`ideation_batches.rs:133-141`), `StateStore::list_proposal_cards_with_actor_ids`
    (`state_store.rs:283-289`, consumed by `project_proposal_cards` :290-318).
  - CLI feeders: `handlers/import.rs:66`; `handlers/check.rs:100`;
    `handlers/check/scope.rs:21,64,71`; `handlers/check/scope/ideation.rs:17-24`.
  - Manifest build/parse sites: `model/manifest.rs:29-30,41`; `scope.rs:78`; `repo.rs:82-86,186-190`;
    `state_store.rs:61-62`.
  - Init flags: `cli.rs:43-47`; `repo.rs:12-13,43-44,82-86` — refuse after the window with wording
    naming `rbac.assignments`.
  - Test fixtures feeding the field (migration blast radius, updated in the same change):
    `crates/provenance-store/src/cache/tests/materialization_behavior.rs:16,52,95`;
    `crates/provenance-store/src/state_store/tests/proposals/lifecycle_validation.rs:449`;
    `.../disposition_write_gate.rs:379`; `.../projection.rs:285`; `.../disposition_allowlist.rs:16-50`;
    `.../legacy_shard.rs:195`; `.../disposition_references.rs:290`;
    `.../ideation_duplicates.rs:53`; `crates/provenance-core/src/model/tests/proposal_lifecycle.rs:68,96,177,285,304,356`;
    `.../proposal_lifecycle_dispositions.rs:27-284`;
    `crates/provenance-cli/tests/cli_import_lifecycle/support.rs:12-22`;
    `crates/provenance-cli/tests/cli_import_legacy_audit.rs:257-268`.
- Non-deadlock property (mandated): after the boundary, no code path consults an allowlist, so a
  manifest with no legacy field and a valid human assignment admits a valid disposition. The old
  "empty list blocks every disposition" posture dies with the field, not beside it.

Test strategy
Window-inside: legacy-only behaves exactly as today (reuse scenario shapes from
`disposition_allowlist.rs:16-50` and `disposition_write_gate.rs:379`; assert the old refusal strings
byte-for-byte); `rbac`-only enforces; both-present refuses with the exact golden; an empty legacy
array beside `rbac` does not refuse. Post-window: a manifest carrying `disposition_actor_ids` fails
parse naming the field; a manifest without it plus a human assignment admits a disposition that the
old law would have blocked (non-deadlock); the retired public APIs no longer exist (compile-level
absence is enough in-crate). CLI tests mirror `crates/provenance-cli/tests/cli_init.rs:89-195`
patterns: warning during the window, refusal after.

Rollout gate
Golden message tests merged; a window-matrix test names all four states explicitly; the removal
change bumps the protocol version and migrates every census line above in one commit.

Complexity L

### W4 — import and merge bypass closure

Goal
No side door: import and the git merge driver must pass the same policy choke as direct writes.

Import (named explicit gate, mandated)
- Fact: import does not write through the primitives. It validates a staged whole-state directory
  and renames it into place under the publication lock
  (`crates/provenance-cli/src/handlers/import.rs:61-93`, staging and swap at :207-286).
- Gate: one explicit call at the top of `import_scope`, inside the existing publication lock, to the
  same `policy.rs` choke: the claim must hold `manifest-write` (census row 17), and every disposition
  in the incoming scope passes the family-12 identity check. The existing aggregate feed
  (:64-72) switches from the allowlist to the W3 replacement at the boundary. The gate stays one
  delegated call (file-size rule above).

Merge driver (mandated closure)
- Fact: `ShardFamily` today recognizes only edges, ideation landings, requirements, and rules; every
  other path merges unchecked (`crates/provenance-store/src/merge/validation.rs:33-47`, `Unrecognized
  => Ok(())` at :112). Fact: when the handler gets no shard path it merges untyped and, with
  `--output`, writes unchecked (`crates/provenance-cli/src/handlers/merge_jsonl.rs:28-40`, write at
  :56-58; the merge itself never inspects fields beyond `id`,
  `crates/provenance-store/src/merge.rs:65-74,76-148`).
- In an `rbac`-managed repository:
  1. Refuse `ShardFamily::Unrecognized` in `validate_merged_records`. Extend typed coverage for the
     remaining canonical families (sources, domains, boundaries, topics, questions, resolutions,
     dispositions, assertions, proposal cards, contributions, synthesis packets, threads, messages,
     requirement reviews, implementation bindings, verification bindings — path shapes at
     `crates/provenance-store/src/shards.rs:99-151`), or refuse whichever stay unrecognized; no
     unchecked family survives under RBAC.
  2. Refuse shard-path-less merge output: when neither `--path` nor a derivable target exists, the
     result cannot be validated, so it refuses (merge_jsonl.rs:28).
  3. Disposition decision-surface checks per row: merged landings (recognized today,
     `merge/validation.rs:104-109`) and a recognized dispositions family run the family-12 identity
     check against the repo's `rbac` section before the merge writes.
  4. Claim transport for automatic Git invocation (mechanically real). Facts: the attribute line
      at `.gitattributes:3` only selects `merge=provenance-jsonl`; git takes no merge-driver
      arguments from attribute lines. The command template is clone-local git config, documented
      at `docs/cli.md:281-291` (`merge.provenance-jsonl.driver "provenance merge-jsonl %O %A %B
      --output %A --path %P"`), mirrored as comments at `.gitattributes.example:4-5`, and git runs
      that template through `sh` (`cli_merge_jsonl.rs:202-204`). The v1 transport for automatic
      invocation is one explicit top-down path: the operator appends a literal `--actor-id <id>`
      argument to the configured driver command at clone setup, for example
      `provenance merge-jsonl %O %A %B --output %A --path %P --actor-id <id>`. The handler parses
      that argv value into one typed `RbacClaim { actor_id }` at the CLI boundary (the same global
      `--actor-id` flag as W2, added to `MergeJsonl` at `cli.rs:268-281`) and everything
      downstream is the W2 choke against the manifest. The transport sniffs nothing: the handler
      never reads environment variables, never runs `git config` itself, never derives the
      principal from `%P` or any path, and no `StateStore` field carries it.
      Setup: the documented one-time per-clone config (`docs/cli.md:284-287`) gains the literal
      `--actor-id <id>`; W6 updates the docs and the `.gitattributes.example:4-5` comment block.
      Update: changing the acting identity means re-running
      `git config merge.provenance-jsonl.driver` with the new literal id in that clone. v1 adds no
      engine verb that writes git config. Between a grant change and that config update, the stale
      literal id resolves as unauthorized and the merge refuses: fail closed.
      Failure: on an `rbac`-managed repository the merge refuses — exiting non-zero so git leaves
      the path unmerged (`merge_jsonl.rs:17-19`, `docs/cli.md:301-303`, proved for real at
      `cli_merge_jsonl.rs:279-294`) — when the configured command supplies no claim (flag absent or
      value empty), an unauthorized claim, an unrecognized shard family, a path-less output, or a
      disposition row failing the family-12 identity check.
      Honesty (no-authentication posture kept): the literal id is an attestation by whoever
      configured the clone, the same posture as the disposition attestation caveat
      (`aggregate_validation.rs:156-158`); v1 adds no authentication. Also a fact, not reopened: a
      clone that never configures the driver falls back to git's line merge, which never invokes
      the handler (`docs/cli.md:289-290`); that pre-existing posture stands and `provenance check`
      remains the detector for it.

Test strategy
Import: an `rbac` repo refuses an import whose claim lacks `manifest-write`, and refuses imported
dispositions whose recorded actor is not human-typed; a valid import succeeds (round trip preserving
the section, `docs/state-format.md:115-120` confirms the manifest travels separately from graph
exports). Merge: an unrecognized shard and a path-less output refuse; a merged landings row carrying
a non-human disposition refuses; a clean typed merge with a valid claim succeeds. Driver transport:
tests configure the real documented driver command through `git config` and run real `git merge`
(pattern `cli_merge_jsonl.rs:176-294`): a command without `--actor-id` on an `rbac` repo fails and
leaves the shard unmerged; a command with an unauthorized literal id fails the same way; a command
with a granted id merges.

Rollout gate
Both bypasses have named refusing tests; the W5 test 15 transport matrix is green; `provenance
check` stays green on merged fixtures.

Complexity M

### W5 — test matrix

Goal
Prove capability enforcement end to end, the legacy boundary, and the mandated cases.

Mandated cases (each becomes a named test)
1. Re-init preserves `rbac` bytes: flags omitted on re-init leave the section untouched (pattern of
   `crates/provenance-cli/tests/cli_init.rs:89-118`).
2. Missing claim differs from wrong principal: the two goldens from W2, asserted as different
   strings.
3. `execute` plus human identity admits a disposition: a human-typed assignment plus a valid claim
   records a disposition on a live proposal; the same call with `agent` or with `identity_type`
   absent refuses with the family-12 golden.
4. Legacy-only unchanged: inside the window, allowlist behavior and refusal strings are
   byte-for-byte today's (`disposition_allowlist.rs:16-50`, `aggregate_validation.rs:200-207`).
5. Both regimes refuse: `ensure_unambiguous_rbac` golden via `StateStore::manifest`, the closed
   projection, and repo parsing (all three readers).
6. Post-window old law cannot deadlock: no legacy field, human assignment present, disposition
   succeeds.
7. Import cannot bypass: W4 import tests.
8. Merge cannot bypass: W4 merge tests.

Matrix carried over from the prior plan, still required
9. Per-capability accept/refuse for each of the four capabilities through a representative census
   row; `read` gates nothing observable in v1 (reads ship ungated; its test asserts parsing and
   harmless presence only).
10. Cross-scope attempts: a principal granted scope A acting on scope B refuses.
11. Swarm landing composition: a batch mixing contributions and dispositions passes or fails per row
    (census row 13, store seam).
12. Default-deny: an unregistered synthetic writer refuses at the primitive backstop.
13. Schema surface: `schema show --artifact manifest` matches its compiled expectation.
14. Init deprecation: warning during the window, refusal after
    (`cli_init.rs:136-195` patterns; seeded-actor CLI pattern
    `crates/provenance-cli/tests/cli_import_legacy_audit.rs:257-268` and
    `crates/provenance-cli/tests/cli_import_lifecycle/support.rs:12-22`).
15. Merge-driver command transport end to end: the configured driver command carries the claim.
    A real `git config merge.provenance-jsonl.driver` without `--actor-id` fails on an
    `rbac` repo and git leaves the shard unmerged; an unauthorized literal id fails the same way;
    a granted id merges (extends `cli_merge_jsonl.rs:176-294`).

Test homes
Store suites under `crates/provenance-store/src/state_store/tests/proposals/` (seeding style:
`lifecycle_validation.rs:449`); core suite
`crates/provenance-core/src/model/tests/proposal_lifecycle_dispositions.rs:27-284` (existing goldens
at :260-261); CLI black-box suites under `crates/provenance-cli/tests/`
(`cli_check.rs:151-153`, `cli_init.rs`). New RBAC unit tests live in the `model/rbac/` test module
(500-line rule).

Rollout gate
Whole matrix green locally and in CI; the count of new tests is recorded in the PR description.

Complexity M

### W6 — docs updates inventory

Goal
Keep user-facing truth aligned with shipped behavior and list the decisions worth recording.

1. `docs/state-format.md` — new paragraph after the manifest sentence at line 5: the `rbac` section
   shape, flat positive-only grants, Git-review-only editing, and the additive no-version-bump
   stance mirroring lines 7-18.
2. `docs/cli.md` — update the init example at line 18; extend the attestation paragraph at lines
   395-399 with the claimed-principal model and `--actor-id`; note near the schema coverage at line
   376 that `schema show --artifact manifest` exists; update the merge driver section
   (`docs/cli.md:270-314`): the documented `merge.provenance-jsonl.driver` template at :284-287
   gains the literal `--actor-id <id>` argument and the setup text at :281-291 explains choosing
   and updating it; sync the comment block at `.gitattributes.example:4-5`.
3. ADR candidates (drafts land in the implementation bead, not here):
   - ADR 0009 candidate: basic RBAC grants in the manifest. Mandatory content: grants are flat and
     positive-only, carry no wildcards, delegation, or expiry, change only through reviewed commits,
     and the engine exposes no verb that writes the section in v1; the legacy window and the
     post-window ratification replacement; no authentication claim
     (`docs/adr/0001-immutable-proposal-lifecycle.md:14` stays true only inside the window, and the
     ADR says when it stops).
4. `CONTEXT.md` domain terms — add principal, resource, capability, assignment only (D1). Editing
   CONTEXT.md belongs to the implementation bead; listed so reviewers see the touchpoint.

Test strategy
Docs-only diff; paste commands from the W3/W5 suites so flag strings match the implemented CLI.

Rollout gate
No doc contradicts shipped behavior; grep for `wildcard`, `delegat`, `expir` in new sections returns
no normative grant features.

Complexity S

## OPEN QUESTIONS

One blocking question (9) goes to human review with this plan. The prior revision's open questions
are disposed:

1 (fallback policy) — settled mechanically: the closed census plus one choke means an unmapped verb
has no capability to check and refuses.
2 (identity encoding) — settled by D2: reuse `IdentityType`, fail closed when absent.
3 (decision-surface enumeration) — settled as dispositions wherever they arrive: direct writes,
swarm batches, imports, merged shards (W4, census row 13).
4 (family split) — the census table above is the reviewed mapping; changes to it go through review,
not this plan.
5 (protocol window) — settled as the SDK protocol bump; numbers are inference (W3).
6 (translation expansion) — moot: no translation exists; legacy stays legacy (D3).
7 (bootstrap) — settled: first init exempt, re-init demands `manifest-write` (census row 20).
8 (state schema version) — stays 1: additive optional field, precedent
`docs/state-format.md:7-18`.

9 (BLOCKING — repo-global scope policy; must be disposed before repo-global implementation)
Facts: the manifest itself and `dictionary.json` sit outside any one scope and affect every scope
then listed; their writers are init's manifest write (`repo.rs:129-133`), the direct dictionary
write (`dictionary_reference.rs:31-37`), and import's staged whole-state swap
(`handlers/import.rs:207-286`). D1 fixes the four capabilities and positive-only explicit scopes;
it does not say how a capability held on scopes governs a resource that spans them. This plan's
every-scope proposal is the fail-safe option below, not a recorded decision, and no binding human
evidence settles one option yet. The human must name one; implementation must not choose.

Alternatives for disposal:
- Option A (this plan's proposal, fail-safe): the capability must be held on every scope then in
  the manifest. Adding a scope narrows who can touch repo-global resources; grants must be
  extended when scopes are added, or import, re-init, and dictionary writes begin refusing.
- Option B: the capability must be held on at least one scope then in the manifest. Simpler
  grants; fail-open across scopes — any single-scope grant confers repo-global power.
- Option C: repo-global mutations refuse on `rbac`-managed repositories in v1. Strongest
  guarantee; makes census rows 17 (import), 19 (dictionary), and 20 (re-init) unusable after
  bootstrap until a later design lands.

Affected census rows: 17, 19, 20, plus the W4 import-gate capability check. Hold: implementation
of these rows does not begin until the human names one option; per-scope rows (1-16, 18) are not
blocked. This decision is not delegated to implementation review.

## ACCEPTANCE CHECKLIST

| Promised outcome | Observable verification |
|---|---|
| Manifest accepts `rbac`; unknown keys inside it refuse | `cargo test -p provenance-core` round-trip and deny tests in `model/rbac/` |
| Old manifests keep parsing everywhere | Existing suites pass unmodified: `cli_check.rs:151-153`, `cli_init.rs`, `state_store/tests/proposals/*` |
| Closed read projection understands the key | `provenance-store` test on `closed_manifest_scope` (`state_store.rs:103-120`) with an `rbac`-bearing manifest |
| One ambiguity refusal, all readers call it | Golden tests on `ensure_unambiguous_rbac` through `StateStore::manifest`, `closed_manifest_scope`, and `repo.rs:186-190`; grep gate in the PR shows no fourth reader |
| Every mutating family maps to a capability; others refuse | W2 census tests; fallback test refusing an unregistered synthetic writer |
| Missing claim and wrong principal refuse differently | W5 test 2 with both goldens |
| Non-human (or untyped) identity cannot end a live proposal | W5 test 3 golden; absent `identity_type` takes the same refusal |
| Legacy-only behavior byte-for-byte inside the window | W5 test 4 asserting today's refusal strings |
| Both regimes refuse; empty legacy array beside `rbac` does not | W5 test 5 goldens |
| Post-window removal cannot deadlock valid dispositions | W5 test 6; retired public APIs absent |
| Re-init preserves `rbac` bytes | W5 test 1 |
| Import and merge cannot bypass | W4 refusing tests for both paths |
| Merge-driver claim transport is mechanically real | W5 test 15: a real `git config` driver command without `--actor-id`, with an unauthorized id, and with a granted id each show their stated outcome |
| Repo-global scope rule comes from the human, not implementation | Question 9 disposed by name (A, B, or C) before any of census rows 17, 19, 20 land; the PR links the disposal |
| No engine verb writes the section in v1 | `--help` snapshot lists no manifest-mutation subcommand; public API grep gate in the PR |
| `schema show` surfaces the section | `provenance schema show --artifact manifest` emits JSON Schema naming `rbac` and the four capabilities |
| No Rust file crosses 500 lines | `wc -l` gate over changed files in CI or PR checklist; extractions in the size table done first |
| Docs match behavior | Sections exist at `docs/state-format.md` line 5 area and `docs/cli.md` lines 18, 376-399; reviewer eyeball plus link check |
| ADR candidate recorded with escalation statement | W6 bullet visible; final ADR lands in its own implementation bead |

## OUT OF SCOPE RESTATED

- Read enforcement of `read`: v1 ships reads ungated. Later attach points, noted so no redesign is
  needed: the reader layer (`StateStore::list_*`, `state_store.rs:146-250`), the closed readers
  module (`crates/provenance-store/src/state_store/readers.rs`, wired at `state_store.rs:50-53`),
  the projection helpers (`project_proposal_cards`, `state_store.rs:290-318`), and the response
  envelope (`crates/provenance-core/src/protocol/response.rs`).
- The SQLite served-read flip (`provenance-1wh`): adjacent work, untouched.
- Authentication, signatures, tokens, identity proofs: excluded for v1 by D1; the actor id stays an
  attestation (precedent `aggregate_validation.rs:156-158`).
- Any external auth-provider/OAuth integration beyond choosing subject-compatible identifier syntax
  (D1).
- Fine-grained authorization: capabilities beyond the four, per-record scopes, delegation, expiry,
  wildcards, revocation tooling, role hierarchies: excluded by D1.
- Migration scripts rewriting manifests: the window is read-side only; the removal change is a wire
  type change, not a data migration (W3).
- Wiki ownership marker and cache artifacts: different files
  (`crates/provenance-cli/src/wiki/publish/manifest.rs:11` describes the `OwnershipManifest`
  generator marker; lock and cache files are non-state per `docs/state-format.md:70-75,99-106`);
  untouched by this plan.

## FACTS AND INFERENCES

Facts (code read this session)

- Core `Manifest` holds exactly `schema_version`, `scopes`, `disposition_actor_ids` with
  `serde(default)` (`crates/provenance-core/src/model/manifest.rs:25-31`); it has no
  `deny_unknown_fields`, so unknown keys are silently ignored today.
- `Manifest::default_with_scope` writes an empty `disposition_actor_ids`
  (`model/manifest.rs:33-43`, `crates/provenance-core/src/scope.rs:78`), and `init` serializes
  whatever the manifest holds (`crates/provenance-cli/src/handlers/repo.rs:87`), so every fresh
  init ships the key with `[]` (`crates/provenance-cli/tests/cli_init.rs:49`).
- `IdentityType { human, agent, service }` exists with serde renames and `parse`
  (`crates/provenance-core/src/model/ideation.rs:74-93`); the CLI disposition handler already
  parses actor type through it (`crates/provenance-cli/src/handlers/dispositions.rs:41`).
- Store reads go through `StateStore::manifest` (`state_store.rs:94-101`) and the
  `deny_unknown_fields` `ManifestProjection` (`state_store.rs:56-63`) used by `closed_manifest_scope`
  (:103-120) via `deserialize_closed` (`readers.rs:259`). Repo parsing is the third reader
  (`repo.rs:186-190`).
- The aggregate validator is the ideation gate (`aggregate_validation.rs:51-115`); the allowlist
  rule, its empty-list posture, its refusal copies, and the attestation caveat live at
  `aggregate_validation.rs:150-207`; the trigger test sits at :376-413.
- `IdeationAggregate.disposition_actor_ids` is fed from `ideation_batches.rs:110,129,139,223`,
  `ideation_writers.rs:115,225`, `proposal_writers.rs:157,184,251`, `handlers/import.rs:66`, and
  `handlers/check.rs:100`; the public actor-keyed APIs are `validate_ideation_scope_with_actor_ids`
  (`ideation_batches.rs:133-141`) and `list_proposal_cards_with_actor_ids`
  (`state_store.rs:283-289`).
- Nearly all shard mutations funnel through `StateStore::mutate_jsonl_records`
  (`publication.rs:458-470`, inside `with_repository_publication` :46-65) or direct
  `mutate_jsonl_locked` calls (`verification_runs.rs:82,130`). Named outliers that bypass both:
  `set_project_dictionary` (`dictionary_reference.rs:31-37`, direct `std::fs::write`), import's
  staged whole-state swap (`handlers/import.rs:207-286`), and init's manifest write
  (`repo.rs:129-133`).
- `ShardFamily` recognizes edges, landings, requirements, rules; everything else merges unchecked
  (`merge/validation.rs:33-47,112`). Merge output without a shard path is untyped and, with
  `--output`, written unchecked (`merge_jsonl.rs:28-58`). `merge_records` inspects only `id`
  (`merge.rs:65-148`). The dispositions shard path
  `.provenance/state/scopes/<scope>/ideation/dispositions.jsonl` is not a recognized family
  (`shards.rs:118-123`).
- Merge-driver wiring has two halves. `.gitattributes:3` carries only
  `.provenance/state/**/*.jsonl merge=provenance-jsonl` and passes no arguments; its comment at
  :2 points at `docs/cli.md`. The command
  template is clone-local git config, documented at `docs/cli.md:281-291` (template at :284-287,
  placeholder contract :293-299, non-zero-exit semantics :301-303) and mirrored as comments at
  `.gitattributes.example:4-5`; git runs the template through `sh`
  (`cli_merge_jsonl.rs:202-204`). `MergeJsonl` CLI args live at `cli.rs:268-281`. The
  documented-driver tests run real `git config` plus real `git merge`
  (`cli_merge_jsonl.rs:176-234` success, :236-294 refusal leaving the shard unmerged).
- The swarm CLI refuses dispositions in merge output (`swarm_backtrace.rs:77-79`), while the store
  API `land_ideation_batch` accepts them (`state_store.rs:70-82`, `ideation_batches.rs:30-39`).
- `SDK_PROTOCOL_VERSION` is 5 with handshake refusal elsewhere (`protocol.rs:25,54-63`) and
  advertisement through `EngineInfo` (`operations.rs:56-63`). `TypedSpecInput` denies unknown
  fields (`crates/provenance-core/src/protocol/typed_spec.rs:18-32`).
- `requirement_reviews` writes ride `mutate_jsonl_records`
  (`requirement_reviews.rs:75,101`) and are invoked from `apply_typed_spec`
  (`typed_specs.rs:232`) and `begin_verification` (`verification_runs.rs:112`).
- File sizes measured this session: `publication.rs` 476; `ideation_batches.rs` 491;
  `shaping_writers.rs` 468; `import.rs` 437; `manifest.rs` 44.
- `init` flags live at `cli.rs:34-48` with semantics at `repo.rs:82-86`; re-init preserve behavior
  is golden-tested (`cli_init.rs:89-118`).
- Docs anchors: `docs/state-format.md:5` (manifest sentence), :7-18 (additive stance), :70-75 and
  :99-106 (cache/lock non-state), :115-120 (graph-reference exclusion); `docs/cli.md:18` (init
   example), :376-389 (schema helpers), :395-399 (attestation), :435 (fog);
  `docs/adr/0001-immutable-proposal-lifecycle.md:14` (allowlist statement).

Inferences (judgment, not code facts)

- "One shared policy choke" is realized as one pure function beside the aggregate validator because
  non-ideation writers never enter aggregate validation today.
- Classifying a resource by shard path inside the primitives is resource identification, not claim
  discovery; the prohibition on path sniffing targets the principal, which only ever comes from the
  explicit claim.
- The window numbers (6 opens, next bump closes) are inference from the current constant; the
  mechanism is settled.
- The every-scope reading of repo-global authorization is this plan's fail-safe proposal and
  Option A of blocking question 9; no binding human evidence settles an option, so disposal is
  review's decision, not implementation's.
- Duplicate `(actor_id, scope)` refusal and the exact refusal wordings are proposed goldens, fixed
  at implementation review.
