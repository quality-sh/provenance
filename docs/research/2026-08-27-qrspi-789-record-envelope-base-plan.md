---
date: 2026-08-27
bead: provenance-789
epic: provenance-46p
stage: plan-pending-human-review
model: glm-5.3-flash-high
---

provenance-789 — Plan: consolidate the record envelope into one shared typed base (epic provenance-46p)

# APPROACH SUMMARY

Human approval fixed the goal from the structure record. This plan fixes how to build it.

Add one owned struct, `RecordEnvelope { schema_version, scope_id, id }`, in `provenance-core`. The open graph-record families replace their first three fields with this struct, flattened, declared first. A small trait, `EnvelopeRef`, exposes the triple to shared consumers such as `GraphNode::id`.

The five ideation families keep their literal fields. Serde does not allow `deny_unknown_fields` beside a flattened member. The structure record flags this as a known coupling (crates/provenance-store/src/operations/plan.rs:18-19). Those structs must stay closed, so they stay out. No closure law weakens.

Nothing moves on the wire. Field names, key order, aliases, defaults, and skip rules stay identical. `SDK_PROTOCOL_VERSION` stays at 5 (crates/provenance-core/src/protocol.rs:25). The store read guard checks raw JSON before any struct builds, so it needs no change (crates/provenance-store/src/state_store/readers.rs:66-99).

Store writers construct through the envelope value instead of three repeated lines. `schema show` output stays hand-written. New equivalence tests compare each published schema against real model fixtures through the `jsonschema` crate already in use (crates/provenance-cli/src/handlers/schema.rs:38-48).

Six work items follow. Each carries its own commit and gate.

# WORKSTREAM BREAKDOWN

## W1 — Add `RecordEnvelope` and `EnvelopeRef` to `provenance-core`

**Goal.** Give the envelope triple exactly one home. Define the type and the accessor trait before any family adopts them.

**Touched files.**

- New file `crates/provenance-core/src/model/envelope.rs`. Register it in the module block at `crates/provenance-core/src/model.rs:1-10`.
- Type inventory used by the new struct: `SchemaVersion` (crates/provenance-core/src/model/ids.rs:26-28), `ScopeId` (:30-34), `StableId` (:68). Ids have no public fields and no other constructors; `new` is the only way in (crates/provenance-core/src/model/ids.rs:12-17).
- Structure shape mirrors today's literal triples, for example `Source` (crates/provenance-core/src/model/artifacts.rs:224-227): bare fields, no serde attributes on the three, derive plain `Serialize`/`Deserialize`.
- Trait `EnvelopeRef`: `fn envelope(&self) -> &RecordEnvelope`, plus thin helpers (`schema_version()`, `scope_id()`, `id()`). Implemented for each adopted family starting in W2.

**Migration notes.** Pure addition; nothing calls it yet. Doc comments must follow the plain English rules (AGENTS.md:31-36). Keep the file small; no Rust file may exceed 500 lines (AGENTS.md:20).

**Test strategy.** Unit tests inside `envelope.rs`: construction accepts valid ids, rejects bad ones, serializes to exactly three keys in declaration order.

**Rollout gate.** `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --all-features -- -D warnings` pass; these mirror CI (.github/workflows/ci.yml:83,97). Full workspace tests green before merge of the slice.

**Complexity.** S/M.

## W2 — Adopt the flattening envelope in the open graph families, slice by slice

**Goal.** Replace the literal first-three-fields pattern in every family without struct-level `deny_unknown_fields`. One commit per family group, so any failure points at one group.

**Touched files.**

| Slice | Records | Citations |
|---|---|---|
| a. artifacts | `Source`, `Requirement`, `Resolution`, `Rule` | crates/provenance-core/src/model/artifacts.rs:224-227, :284-287, :355-358, :404-407 |
| b. shaping | `Boundary`, `Topic`, `Question` | crates/provenance-core/src/model/shaping.rs:113-116, :125-128, :142-145 |
| c. services | `Domain` | crates/provenance-core/src/model/services.rs:6-9 |
| d. collaboration | `Thread`, `Message` | crates/provenance-core/src/model/collaboration.rs:44-47, :54-57 |
| e. integrations | `VerificationRun`, `VerificationBinding`, `RequirementReview`, `ImplementationBinding` | crates/provenance-core/src/model/integrations.rs:107-110, :132-135, :153-156, :172-175 |
| f. edge | `Edge` | crates/provenance-core/src/model/graph.rs:77-81 |

Each struct gains `#[serde(flatten)] pub envelope: RecordEnvelope,` as its first field and loses the three literals. Each struct implements `EnvelopeRef` trivially. Serde derives stay on the outer structs.

**Migration notes.**

- Key order survives: flatten forwards the inner struct's fields where the member sits, and first position keeps `schema_version, scope_id, id` ahead of everything else. Treat this as an assumption under test (see INFERENCE).
- Internal field reads migrate mechanically: `record.id` becomes `record.envelope.id` or the trait helper `record.id()` where ergonomics favor it.
- The canonical format keeps top-level stable string `id` and `schema_version` per shard (docs/state-format.md:3); flatten preserves that because there is no nested object on the wire.
- No alias additions and no renames on the three; searches show no envelope-field aliases exist today (see FACTS).
- Expect compile errors at every construction site. Do not fix writers here; W5 owns them. Land model plus minimum site repairs per commit when required to compile.

**Test strategy.** Existing byte-level pins must pass without edits: camelCase-in/snake-out with an unchanged `schema_version` (crates/provenance-core/src/model/tests/artifacts.rs:45-96, same family suite at :118 for resolution and rule), shaping (crates/provenance-core/src/model/tests/shaping.rs:7-90), services (crates/provenance-core/src/model/tests/services.rs:4+), collaboration (crates/provenance-core/src/model/tests/collaboration.rs:4+), integrations method words (crates/provenance-core/src/model/tests/integrations.rs:4+). Add one explicit byte-diff fixture per slice: serialize before-commit output against after-commit output for one representative record and assert equality.

**Rollout gate.** After each slice: the slice's pins, then `cargo test --workspace`, then the fmt/clippy gate from W1. Any red pin stops the batch; revert the slice; do not weaken a pin.

**Complexity.** L (many call sites across crates; low risk each, high coordination cost).

## W3 — Keep the closed ideation families out, and say why

**Goal.** Make the exclusion explicit and auditable. The five ideation carriers keep their literal triples and their `deny_unknown_fields`.

**Touched files.**

- Excluded structs and their closure attributes: `Contribution` (crates/provenance-core/src/model/ideation/contributions.rs:64-69), synthesis payloads and packet, proposal inputs and cards, dispositions, and `AssertionRecord` (crates/provenance-core/src/model/ideation/lifecycle.rs:38-43; full attribute list in the structure record's finding 3).
- Documentation lives in `envelope.rs` doc comments from W1: name the two rules in tension — structural denial per struct versus one shared flattened base — and why denial wins here. Cite the house note (crates/provenance-store/src/operations/plan.rs:18-19) as prior art.

**Migration notes.** None. Zero behavior change. Because `AssertionRecord` stays out, no `AssertionId` decision blocks this bead (`AssertionId` remains a transparent newtype over `StableId`, crates/provenance-core/src/model/ideation/lifecycle.rs:20-22).

**Test strategy.** Ideation pins stay green untouched (crates/provenance-core/src/model/tests/ideation.rs:10-150 area, plus disposition suites). Add one negative unit proof: attempt to opt a `deny_unknown_fields` struct into flatten fails to compile under a `compile_fail` doctest or an ignored illustrative example, recording the upstream limit.

**Rollout gate.** Workspace tests and fmt/clippy gates green; no ideation file appears in the slice diff except comments.

**Complexity.** S.

## W4 — Route shared consumers through `EnvelopeRef`

**Goal.** Deduplicate kind-matched plumbing where the seam fits. Do not chase every match arm — only ones the envelope genuinely serves.

**Touched files.**

- `GraphNode::id()` collapses six arms to one trait call (crates/provenance-core/src/protocol/node.rs:39-48). The internally tagged union itself stays as-is (crates/provenance-core/src/protocol/node.rs:16-25).
- Leave `retired()` (:54-61) and `searchable_text()` (:64-88) matched per kind; they read non-envelope content, so the trait cannot serve them honestly.
- `validate_source_schema_versions` reads contributions and packets directly today (crates/provenance-core/src/model/ideation/lifecycle/aggregate_validation.rs:117-122). It guards excluded families, so leave it alone despite its misleading name.
- The store version guard stays raw-JSON by design, before any struct builds (crates/provenance-store/src/state_store/readers.rs:66-99, function at :86). The single-home constant and its enforcement remain untouched (crates/provenance-core/src/model/ideation/lifecycle/aggregate_validation.rs:13-19, :41-48).
- The CLI artifact validator surfaces serde messages from the types themselves (crates/provenance-cli/src/handlers/validate.rs:40-77). Types it handles for adopted families now resolve envelope fields internally; no signature changes.

**Migration notes.** Behavior-preserving refactor only. Serde message text for missing envelope fields may shift shape behind flatten on adopted families; the validator passes messages through. Disclose this in the PR body rather than papering over it.

**Test strategy.** Query wire bytes stay pinned: the response `node.node_type` assertion (crates/provenance-cli/tests/cli_sdk_query_get.rs:22) and neighbor/walk responses. Run the query-integration groups touching `GraphNode`.

**Rollout gate.** Suite-level identity: all CLI query tests green; fmt/clippy gates green; no pin edited.

**Complexity.** M.

## W5 — Absorb the envelope into store-side constructors

**Goal.** Every writer of an adopted family sets `envelope: RecordEnvelope::new(...)?` once instead of three fields. Fail closed where ids or versions fail validation, using existing validation.

**Touched files.** Census first, then edit. Seventeen non-test store files assign `schema_version: SUPPORTED_SCHEMA_VERSION` today; the representative shape is exhaustive struct-literal construction with the triple first:

- Example: `Resolution` literal (crates/provenance-store/src/state_store/rule_writers.rs:40-59).
- Threads and messages (crates/provenance-store/src/state_store/thread_writers.rs:45, :69), bindings (crates/provenance-store/src/state_store/verification_bindings.rs:51).
- Others in the same census: domain_writers.rs, shaping_writers.rs, writers.rs, verification_runs.rs, requirement_reviews.rs, implementation_bindings.rs, publication.rs, operations.rs (census path: `rg -l 'schema_version: SUPPORTED_SCHEMA_VERSION' crates/provenance-store/src --include=*.rs`, minus tests).
- Out of this item on purpose: `graph_reference` export writers build plain-`u32` versions with `.0` coercion (crates/provenance-store/src/graph_reference.rs:195-242); see Open Question 3.

**Migration notes.**

- Struct-update syntax cannot splat a subfield; each literal's first three lines become one constructor call. Mechanical, signature-safe edits.
- Constructor returns `Result`; sites already sit inside fallible closures, but review each for plumbing (most already validate ids upstream).
- Round-trip safety: every write re-reads the shard first and refuses unsupported rows pre-mutation (crates/provenance-store/src/state_store/readers.rs:66-99). The byte behavior of written rows comes from the same serde derives pinned in W2.

**Test strategy.** Store integration suites covering writes and reads per family; the seventeen-family refusal suite stays green untouched (crates/provenance-cli/tests/cli_record_schema_versions.rs:5-32 census, :34-72 refusal loop); cache materialization fixtures that plant raw shard rows keep passing (crates/provenance-store/src/cache/tests/materialization_behavior.rs around :273-304).

**Rollout gate.** Per-file commits grouped by writer module. Gate after each: `cargo test -p provenance-store -p provenance-cli`, then the fmt/clippy pair from W1. Whole-workspace test before marking the item done.

**Complexity.** L (breadth of files; mechanical risk profile).

## W6 — Close the schema-drift gap with fixture equivalence tests

**Goal.** Stop hand-written published schemas from drifting from the Rust types. Keep builders hand-written; add executable equivalence checks. No generator dependency lands in this bead.

**Touched files.**

- Published schemas and dispatch: `schema_for` maps seven kinds, all `IdeationArtifactKind` variants (crates/provenance-cli/src/handlers/schema.rs:19-36). Builders hand-write envelopes, e.g. required `["schema_version","scope_id","id",...]` and `"const": 1` (crates/provenance-cli/src/handlers/schema/artifacts/contribution.rs:5-46).
- Existing drift net covers enums only: exhaustive variant arrays (crates/provenance-cli/src/handlers/schema/tests.rs:141-169) and enum values against model serialization (:172-266). The export schema alone validates whole fixtures today (fixture builder :295-344; named test `graph_reference_export_schema_validates_record_structure` at :346).
- Extension home: same tests module, alongside the enum suites.

**Method.**

1. Build one minimal valid record per kind from the model structs, not by hand-writing JSON.
2. Assert `jsonschema::JSONSchema` compiled from `schema_for(kind)` accepts the serialized record (compile path already exercised at crates/provenance-cli/src/handlers/schema.rs:38-48).
3. Mutate the fixture three ways: drop one required key, add an unknown key, bump `schema_version` past 1. Assert rejection each time for schemas that declare `additionalProperties: false` or a `const` version.
4. Compare the schema's required-and-properties key set against the serialized record's key set, so a renamed or dropped Rust field fails loudly even when acceptance still holds.

**Migration notes.** Tests only. Note plainly in test comments that these schemas describe kinds whose structs kept literal fields (W3), so this net covers them from outside.

**Test strategy.** The Method paragraph above states the whole strategy. All new cases go under `handlers/schema/tests.rs`.

**Rollout gate.** `cargo test -p provenance-cli handlers::schema` green; full workspace test; fmt/clippy gates green.

**Complexity.** M.

Suggested landing order: W1, W2a-f interleaved with W5 slices as compile demands require, W3 whenever the boundary needs recording (any time after W1), W4 and W6 last, since they ride on stabilized shapes. Total slices: nine to eleven commits, each gated.

# OPEN QUESTIONS FOR HUMAN REVIEW

1. **Exclusion mode confirmed?** This plan excludes the five closed ideation families entirely (W3). The structure record offered an alternative: re-home their closure onto pre-read inspection like the store guard. Confirm exclusion; the plan does not assume re-homing anywhere.
2. **AssertionId left alone?** Because `AssertionRecord` stays out, unifying `AssertionId` into `StableId` buys nothing this bead. Plan defers it completely (W3 notes). Confirm deferral.
3. **graph_reference plain u32 deferred?** Export structs carry `schema_version: u32` (crates/provenance-store/src/graph_reference.rs:30, :61, :69) and `GraphReference` itself declares `deny_unknown_fields` (:28-29), so a flattened base cannot land there anyway. Promoting `u32` to `SchemaVersion` would be typing churn without deduplication. Confirm deferral to a future bead.
4. **Schema sync route confirmed?** Plan lands route one only: equivalence tests (W6). Committed generation following the npm platform pattern (.github/release-targets.json feeding packages/provenance/scripts/package-engine.js:4-31, guarded by .github/scripts/release-contract.test.py) stays out, including any new dependency like schemars absent from Cargo.lock today. Confirm, or cut a child bead now.
5. **Error-text softening accepted?** Behind flatten, serde diagnostics for missing envelope fields may lose some pointer precision on adopted families, and the CLI validator prints serde messages straight (crates/provenance-cli/src/handlers/validate.rs:40-77). Acceptable? If not, W2 stops and falls back to macro-stamped declarations for affected slices (structure Candidate B), preserving bytes and messages at the cost of a shared type object.
6. **Epic corrections recorded elsewhere?** Two conversation-context anchors proved stale during research: the TypeScript mirror never spells the per-record envelope (packages/provenance/src/protocol.ts:197-203 shows an index-signature form carrying only `id` explicitly; TypedSpecDocument at :59-67 pins `schema_version: 1` alone), and the generator script lives under packages/provenance/scripts/, not scripts/. This bead plans zero TS edits. Confirm someone updates the epic description outside this bead.

# ACCEPTANCE CHECKLIST

Each promised outcome maps to one observable verification. All verifications run from the repository root unless stated.

1. **One shared typed base exists exactly once.**
   Verify: `rg -n "pub struct RecordEnvelope" crates/provenance-core/src/model/envelope.rs` returns one definition; the module appears in the block at crates/provenance-core/src/model.rs:1-10; no second copy of the triple-as-type elsewhere (`rg -n "pub struct RecordEnvelope" crates/` finds one).
2. **Open families carry no duplicated literal triple.**
   Verify: `rg -n "pub schema_version: SchemaVersion" crates/provenance-core/src/model/artifacts.rs crates/provenance-core/src/model/shaping.rs crates/provenance-core/src/model/services.rs crates/provenance-core/src/model/collaboration.rs crates/provenance-core/src/model/integrations.rs crates/provenance-core/src/model/graph.rs` returns zero matches; each struct instead has one flattened member and one-line `EnvelopeRef` impl.
3. **Closed families still closed.**
   Verify: `rg -n "deny_unknown_fields" crates/provenance-core/src/model/ideation/` returns the same pre-change hits (baseline captured before this bead starts: contribution and lifecycle attributes, e.g. contributions.rs:65, lifecycle.rs:39); no ideation struct gains flatten.
4. **Wire bytes unchanged on adopted families.**
   Verify untouched-green: camel-in/snake-out pins (model/tests/artifacts.rs:45-96, :118+), shaping/services/collaboration/integrations pins listed in W2, disposition closure test (model/tests/proposal_lifecycle_dispositions.rs:149). Diff shows zero edits to those test functions.
5. **Query responses keep their shape.**
   Verify: crates/provenance-cli/tests/cli_sdk_query_get.rs:22 assertion green; its file shows no edits.
6. **Version governance unmoved; protocol unmoved; no migration.**
   Verify: `git diff main...HEAD -- crates/provenance-core/src/protocol.rs` empty at line 25 region (`SDK_PROTOCOL_VERSION` stays 5); crates/provenance-cli/tests/cli_record_schema_versions.rs:34-72 green without edits; docs/state-format.md untouched; no migration code added (`rg -n "migrat" crates/` adds no production hits).
7. **Shared consumers de-duplicated.**
   Verify: `GraphNode::id()` body is a single trait call, no six-arm match (inspect crates/provenance-core/src/protocol/node.rs post-change); `retired()` and `searchable_text()` retain their documented per-kind matches.
8. **Writers stopped repeating the triple.**
   Verify: the census command from W5, rerun after adoption, lists only files building excluded-family or deliberately-deferred records (ideation/proposal/lifecycle writers, graph_reference export paths); each remaining adopted-site occurrence replaced by one `RecordEnvelope::new` call (`rg -n "RecordEnvelope::new" crates/provenance-store/src` count equals number of adopted-construction sites reviewed in the PR description).
9. **Published schemas cannot drift silently.**
   Verify: new tests exist for all seven kinds (count case groups in crates/provenance-cli/src/handlers/schema/tests.rs; seven positive acceptances, three negative mutations each where applicable); temporarily renaming a builder key in a scratch checkout makes the new suite fail — demonstrated once in the PR description, then reverted.
10. **House standards held.**
    Verify: no touched Rust file exceeds 500 lines (`wc -l` over the changed set; cap at AGENTS.md:20); new doc comments and this document use plain short sentences per AGENTS.md:31-42; `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets --all-features -- -D warnings` clean, matching .github/workflows/ci.yml:72-97.

# OUT OF SCOPE RESTATED

From the question-and-structure record (docs/research/2026-08-27-qrspi-789-record-envelope-base.md:33-39), unchanged:

- No change to the canonical state format and no change to `SUPPORTED_SCHEMA_VERSION`'s value (single home stays at crates/provenance-core/src/model/ideation/lifecycle/aggregate_validation.rs:19).
- No `SDK_PROTOCOL_VERSION` bump and no new wire capability.
- No added or removed record families; no renamed wire fields; no camelCase aliases on envelope fields.
- No change to what the `Validate` command accepts beyond current behavior.
- Code execution stays out until this plan clears human review; child issues are cut elsewhere.

Added by disposal evidence in the structure record, restated so the reviewer sees them consciously buried:

- No plain nesting of a base object behind a named field; Candidate D died on the top-level-shape pins (docs/state-format.md:3; schema/export fixtures at handlers/schema/tests.rs:295-344).
- No removal of `deny_unknown_fields` from any closed struct.
- No schema generation pipeline, no committed generated artifacts, and no new dependencies in this bead (Open Question 4 may spawn a child bead).
- No TypeScript edits expected or planned.

# FACTS VERSUS INFERENCE

## Repository facts (each verified at a citation)

- Fifteen open carriers repeat the envelope triple as their first three declared fields, itemized with exact ranges in the W2 census table; the closed ideation carriers repeat it too, behind their deny attributes; seventeen stored kinds appear in the CLI refusal census (crates/provenance-cli/tests/cli_record_schema_versions.rs:5-32).
- Only `AssertionRecord.id` deviates among stored triples, holding a transparent `AssertionId` over `StableId` (crates/provenance-core/src/model/ideation/lifecycle.rs:20-22, :38-43); only the graph-reference export structs use plain `u32` versions (crates/provenance-store/src/graph_reference.rs:30, :61, :69).
- The ideation family closes at the struct level with `deny_unknown_fields` (contributions.rs:65; lifecycle.rs:39; more in the structure record's finding 3); open families carry no such attribute at any audited site.
- The house limit connecting flatten and denial is stated in-tree (crates/provenance-store/src/operations/plan.rs:18-19), and no current struct combines both on itself (grep audit across crates/).
- `GraphNode` is internally tagged with per-arm accessors (crates/provenance-core/src/protocol/node.rs:16-25, :39-48, :54-61, :64-88).
- The version guard runs on raw JSON before structs build, with the reasoning in its own doc comment (crates/provenance-store/src/state_store/readers.rs:66-99, function at :86); the supported-version constant is single-homed and self-described as having no second copy (aggregate_validation.rs:13-19).
- `SDK_PROTOCOL_VERSION` is 5 (crates/provenance-core/src/protocol.rs:25); ADR lineage splits wire from canonical versions per the structure record's finding 6.
- Byte-pinning round-trip tests exist per family (artifacts.rs:45-96, :118; shaping.rs:7; services.rs:4; collaboration.rs:4; ideation.rs:10; integration methods :4; query node shape at cli_sdk_query_get.rs:22).
- `schema show` publishes seven hand-written schemas keyed to `IdeationArtifactKind` (handlers/schema.rs:19-36), hard-codes the envelope mirror (contribution.rs:5-46), compiles `jsonschema` already (handlers/schema.rs:38-48), and its drift net today covers enums only (tests.rs:141-169, :172-264) plus whole-fixture export validation (tests.rs:295-346).
- Writers build records as exhaustive literals, triple first, in seventeen non-test store files, exemplified by rule_writers.rs:40-59; thread_writers.rs:45, :69; verification_bindings.rs:51; graph_reference.rs:195-242.
- House constraints: 500-line Rust cap (AGENTS.md:20), ASD-STE100-style prose for technical text (AGENTS.md:31-36), CI fmt/clippy commands (ci.yml:72-97).
- The TypeScript mirror never spells the envelope triple; `GraphNode` carries an index-signature form (protocol.ts:197-203) and `TypedSpecDocument` pins only `schema_version: 1` (protocol.ts:59-67).

## Inference (cannot be proven from the repository alone)

- Serde's derive rejects `deny_unknown_fields` combined with a flattened member on one struct. The in-tree note implies it indirectly; plan.rs documents a workaround culture, not the compiler diagnostic. W1 includes a compile-time spike before W2 adopts anything.
- Flatten preserves the inner struct's serialized key order at the member's position, so declaring the envelope first keeps today's `schema_version, scope_id, id` ordering on disk and in responses. Treated as an assumption under the byte-equality fixtures W2 requires per slice.
- Serde diagnostics for missing envelope fields change wording behind flatten. Humans read these through the CLI validator; if reviewers reject softer text, W2 falls back toward Candidate B mechanics per Open Question 5.
- Fixture equivalence testing suffices to hold hand-written schemas honest for the seven published kinds. The existing enum-drift discipline supports it, but coverage of property-name sets is new and unproven until written.
- Human approval fixed the WHAT as a genuine shared type object (Candidate A) bounded to the open families. This reading drives W1-W3; Open Question 1 puts the bounding back in front of the reviewer deliberately.
