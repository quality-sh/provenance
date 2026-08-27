---
date: 2026-08-27
bead: provenance-789
epic: provenance-46p
stage: structure-complete-awaiting-disposal
model: glm-5.3-flash-high
---

provenance-789 — Consolidate the record envelope into one shared typed base (epic provenance-46p)

=== QUESTION ===

How should the repeated per-record envelope (`schema_version`, `scope_id`, `id`) become one shared Rust type across the record families in `provenance-core` (and the store's writers), and how should the published per-kind JSON Schemas (`provenance schema show`) be kept synchronized with that Rust truth?

Sub-questions:

1. Which record families actually carry the envelope, and do they differ in id type (`StableId` vs `AssertionId` vs plain `u32`) enough that one struct cannot fit all?
2. Can a shared base be added **purely internally** — no canonical schema_version bump, no SDK `protocol_version` increment (currently 5), no migration — or do the round-trip pins refute that?
3. How does any shared-base mechanism survive three specific serde hazards: (a) structs that carry `#[serde(deny_unknown_fields)]` themselves (the entire ideation family), (b) the internally-tagged union `GraphNode` where `tag = "node_type"` coexists with record content, and (c) the alias-on-input / snake_case-out convention pinned by model tests?
4. Does consolidating storage vs consolidating logic distinguish the candidates? (field deduplication vs accessor/validation deduplication are separable goals)
5. What mechanism keeps `schema show`'s hand-written JSON Schemas from drifting from the Rust structs after the refactor — regeneration (house precedent: npm platform packaging), extended equivalence tests, or both?
6. Do the TypeScript mirrors under `packages/provenance/src/protocol.ts` shrink mechanically afterward, as the conversation context asserts?

In scope:

- `crates/provenance-core/src/model/**` record structs carrying the envelope triple
- Store-side construction sites in `crates/provenance-store/src/state_store/*writers*`, `graph_reference*`
- `GraphNode` internals (`protocol/node.rs`) and everything that consumes records generically (`validate_source_schema_versions`, CLI write gates, `handlers/schema.rs`)
- `schema show` schemas (`handlers/schema/artifacts/*`) and their drift tests (`handlers/schema/tests.rs`)
- Typed-spec wire pins (`rule_rust_typed_input_round_trip`) insofar as they constrain mechanism choice
- Whether TS mirrors need touching at all

Out of scope:

- Changing the canonical state format or `SUPPORTED_SCHEMA_VERSION` value
- Any `SDK_PROTOCOL_VERSION` change or new wire capability
- Adding/removing record families, renaming fields on the wire, adding camelCase aliases to envelope fields
- Shaping decisions about which artifacts the `Validate` command accepts beyond what exists
- Writing any code, plan document, or child issue (this session stops here)

The answer must enable a human reviewer to decide: (i) which consolidation mechanism (flatten-shared-type vs macro-stamped-declaration vs trait-only) fits the repo's fail-closed laws, (ii) whether the ideation family's struct-level `deny_unknown_fields` may ever coexist with a shared flattened base, or whether those families stay out of the consolidation, (iii) whether `schema show` output becomes generated-and-committed like npm platform artifacts, and (iv) confirmation that this refactor carries no version/migration obligation.

=== RESEARCH ===

**1. Envelope repetition census.** Every stored record family declares the identical triple as its first three fields: `Source` (artifacts.rs:224-227), `Requirement` (:285-287), `Resolution` (:356-358), `Rule` (:405-407); `Boundary`, `Topic`, `Question` (shaping.rs:113-116, 125-128, 142-145); `Domain` (services.rs:6-9); `Thread`, `Message` (collaboration.rs:43-47, 54-57); four integration records (integrations.rs:108-110, 133-135, 154-156, 173-175); `Edge` (graph.rs:77-81); `Contribution` (ideation/contributions.rs:66-69); `SynthesisPacket` (ideation/synthesis.rs:85-90); `ProposalCard` (ideation/proposals.rs:22-27); `DispositionRecord` (ideation/dispositions.rs:32-36); `AssertionRecord` (ideation/lifecycle.rs:38-43). `cli_record_schema_versions.rs:5-32` enumerates 17 stored families including legacy dispositions — matching repetition count.

**2. Id-type variance inside the "identical" triple.** Most families use `pub id: StableId`. Two deviations exist: `AssertionRecord.id: AssertionId`, a transparent newtype over `StableId` (lifecycle.rs:20-22, 40-43), and the graph-reference export structs use `schema_version: u32` directly rather than `SchemaVersion` (graph_reference.rs:30, 61, 69). One shared base struct cannot contain `AssertionId` and `StableId` simultaneously without either unifying those types or parameterizing the base.

**3. Closure asymmetry — the load-bearing constraint.** The ideation family is closed *at the model level*: `Contribution` (contributions.rs:64-66), all synthesis payloads and `SynthesisPacket` (synthesis.rs:8-9 through 86-88), proposal inputs/cards (proposals.rs:10, 21-22), disposition records (dispositions.rs:7, 16, 23, 32), `AssertionRecord` (lifecycle.rs:38-40), `IdeationTarget` and `IdeationEvidenceReference` (ideation.rs:274-276, 283-285) all declare `#[serde(deny_unknown_fields)]`. By contrast `Source`/`Requirement`/`Resolution`/`Rule`/shaping/collaboration/integration records have no such attribute. Deny-closure for *those* families lives only in the read guard (`state_store/readers.rs:60-77` refuses unsupported-version rows for every family before serde gets them) and in the closed protocol-input layer (typed_spec.rs:19-107, query.rs:41-161 ×8 structs, state_store/inputs.rs:214-257). So an attribute-level collision exists for five-plus ideation structs but not for the graph families.

**4. In-repo flatten precedent — including a documented limit.** `#[serde(flatten)]` is already used eight places: `QueryResponse<Result>` flattens its result generically (protocol/response.rs:16-23), plus coverage.rs:174, statement_policy.rs:14, dictionary.rs:13, proposal_surfaces.rs:83, prime.rs:9, operations/plan.rs:22 and :29. Crucially, plan.rs:16-19 states the house understanding of the interaction: "`TypedSpecResult` flattens ..., so `TypedSpecResult` must never gain `deny_unknown_fields`" — flatten and deny-on-the-flattened-target don't compose there either. No current struct combines `flatten` with `deny_unknown_fields` on itself.

**5. Internally-tagged union and generic-consumer duplication.** `GraphNode` is `#[serde(tag = "node_type", rename_all = "snake_case")]` over boxed full records (protocol/node.rs:16-25); every field of each variant therefore must keep deserializing correctly *under serde's internal content buffering*. Its accessors duplicate per-kind matching today: `id()` (:39-48), `retired()` (:54-61), `searchable_text()` (:64-88) — the natural beneficiaries of an envelope trait. The store guard, notably, reads `schema_version` from the **raw JSON**, not from a struct, explicitly because "the struct is what we refuse to build until the version is known" (readers.rs:79-100) — so the version gate is immune to Rust-shape changes by construction.

**6. Version governance is already single-homed; protocol lineage supports "internal-only".** `SUPPORTED_SCHEMA_VERSION: SchemaVersion = SchemaVersion(1)` is declared once with prose stating there is "no second copy of the number to update" (aggregate_validation.rs:13-19); `ensure_supported_schema_version` enforces it via `rule_schema_version_one` (:41-49) and runs at aggregate level for contributions/synthesis/proposals (:117-129), dispositions (:273), and assertions (assertion_validation.rs:25); the CLI artifact validator delegates to the same functions (handlers/validate.rs:46-76); `cli_record_schema_versions.rs:36-72` proves v2 refusal across all 17 families. ADR 0008 fixed the pattern for prior changes: "The wire change increments the SDK protocol from 4 to 5. The canonical state schema stays at version 1" (docs/adr/0008:40-41); `SDK_PROTOCOL_VERSION = 5` (protocol.rs:25). docs/state-format.md:3,7 pins shard shape ("stable string `id` fields, `schema_version`") at version 1, and adoption-related wire work previously landed with "state schema version 1 needs no migration" (docs/state-format.md:17-18).

**7. The round-trip pins that decide sub-question 2.** Model tests pin alias-in/snake-out byte behavior per family, with names saying exactly what they pin: `enriched_source_and_requirement_records_roundtrip_without_schema_bump` decodes `"commitPin"`/`"effectiveDate"` camelCase and asserts snake_case out plus unchanged `schema_version` (model/tests/artifacts.rs:44-97); retirement omission/explicitness pinned at :3-42; equivalent fixture tests exist for shaping/services/collaboration/ideation (model/tests/shaping.rs:8-90, services.rs:5-28, ideation.rs:11-150). Typed-spec input round-trip is pinned stricter — serialize→decode→serialize equality under `#[verifies("rule_rust_typed_input_round_trip", examples)]` (authoring/tests.rs:301-314) against `TypedSpecInput` with `deny_unknown_fields` (typed_spec.rs:17-19). Note two corrections to the conversation context: `TypedSpecInput.schema_version` is a plain `u32 = 1` and the struct has **no** `scope_id`/`id` (typed_spec.rs:20-31), so it is not itself an envelope carrier; and query responses wrap records inside an encode-only flatten envelope keyed by `protocol_version`+`operation` (response.rs:11-23) whose exact bytes are asserted in tests like cli_sdk_query_get.rs:22 (`answer["node"]["node_type"]`). Conclusion supported: an envelope refactor that preserves field names, positions, aliases, defaults, skip-rules, and deny semantics touches no pin listed above — nothing in the pins depends on *how many source-line repetitions* produce the fields.

**8. Published JSON Schemas are hand-written mirrors, partially drift-tested.** `schema show` dispatches per kind to hand-written `json!({...})` builders — e.g., contribution schema hardcodes required `["schema_version","scope_id","id",...]` and `"schema_version": {"const": 1}` (handlers/schema/artifacts/contribution.rs:5-45) — wrapped by schema_for (handlers/schema.rs:19-36). Drift protection today covers enum vocabularies only: exhaustive variant-array checks (handlers/schema/tests.rs:140-169) and enum values matched against model serialization (:172-264). The graph-reference-export schema additionally validates complete record fixtures including the envelope (tests.rs:307-342) and rejects envelope-adjacent errors like `"schema_version": 2` or an unexpected `origin_thread` (:368-415). No test anywhere compares a schema's property-name set against the corresponding Rust struct. `jsonschema::JSONSchema` is already used for compile-and-validate (handlers/schema.rs:40-46), so tooling for fixture-driven equivalence is present.

**9. House generation precedent.** The npm platform packaging shows the accepted pattern: one JSON source of truth (`.github/release-targets.json`, release.yml:95 passes it as `--targets`), a generator consuming it (`packages/provenance/scripts/package-engine.js:4-31` reads the targets file and writes committed platform manifests), and release contract tests guarding the relationship (`.github/scripts/release-contract.test.py`). Note the context anchor was stale on the generator's path — no `scripts/package-engine.js` exists at repo root.

**10. Contradicting evidence found while verifying context claims.**
(a) The claim that "the TypeScript mirrors shrink mechanically afterward" is largely false: `packages/provenance/src/protocol.ts` never spells out the per-record envelope — `GraphNode` is `{ node_type; id; retired?; [field: string]: unknown }` (protocol.ts:197-203) and `TypedSpecDocument` carries only `schema_version: 1` (:59-67). At most, envelope consolidation forces zero TS edits.
(b) None of the three envelope fields carries a camelCase alias anywhere (grep for `alias = "scopeId"/"schemaVersion"/"nodeId"` returns nothing) — aliases live only on non-envelope fields.
(c) Production writers construct records via exhaustive struct literals with the triple first (rule_writers.rs:40-59; ideation_writers.rs:49-50 uses `SUPPORTED_SCHEMA_VERSION`; thread_writers.rs:45,69; verification_bindings.rs:51; graph_reference.rs:195-242), so a move-to-base refactor mechanically touches every writer unless struct-update syntax absorbs it.

Evidence-split checklist —

Repository facts (cited above): the ~17-family repetition; `AssertionId`/plain-u32 id variance; ideation-family struct-level closure vs graph-family openness; the plan.rs:16-19 flatten/deny note; the raw-JSON version guard; single-homed `SUPPORTED_SCHEMA_VERSION` and its call sites; ADR 0008's wire-vs-canonical version split; per-model-file round-trip pins; hand-written schema builders with enum-only drift tests; npm packaging generation pattern; TS mirror's index-signature shape; absence of envelope-field aliases; writer construction sites.

My inference (not provable from the repo alone): serde derive refuses `deny_unknown_fields` together with `flatten` on the same struct (well-known upstream limitation, reflected indirectly in plan.rs:16-19 but never triggered here); flatten forwards serialized fields in declaration order, so a first-position flattened base preserves today's JSON key order; internally-tagged deserialization buffers content harmlessly for these self-describing types; and the error-text surface for missing envelope fields would shift if they moved behind flatten (affects validator diagnostics humans see). Each inference needs a compile-time spike before implementation — none changes the structuring below.

=== STRUCTURE ===

**Candidate A — Shared `RecordEnvelope` struct, flattened first member, accessor trait (full type consolidation).**

Position: define one owned type `RecordEnvelope { schema_version: SchemaVersion, scope_id: ScopeId, id: StableId }`; every graph-family record replaces its first three fields with `#[serde(flatten)] pub envelope: RecordEnvelope` declared first; a small trait `EnvelopeRef { fn envelope(&self) -> &RecordEnvelope }` exposes the triple, and cross-kind consumers (`GraphNode::id/retired/searchable_text`, node.rs:27-88; `validate_source_schema_versions`, aggregate_validation.rs:117-135; CLI gates) match through it. For the ideation families, only `id` typing blocks literal sharing — unify `AssertionId` toward `StableId` (it is transparent anyway, lifecycle.rs:20-22) or exclude those five structs.

Mechanism sketch (interface/invariant level): the base owns no behavior beyond the three fields and a constructor taking validated ids; ids can only be built through `new` (ids.rs:14-17 keeps that invariant). Serde derives stay on the outer structs. Because flatten preserves the flattened struct's field names and it sits in first position, serialization order matches today's `schema_version, scope_id, id` prefix. The deny-unaware graph families adopt it wholesale; the ideation families' struct-level `deny_unknown_fields` **cannot** sit next to `flatten` (repo-documented coupling at plan.rs:16-19; believed compile-time rejection) — so Candidate A either (a) applies to non-closed families only, (b) drops deny from those structs, unacceptable because it converts refuse-at-the-door into silent acceptance, or (c) re-homes closure for the ideation family onto `readers.rs`-style pre-read inspection — a real design fork with message-text consequences (validate.rs today surfaces serde messages directly, handlers/validate.rs:47-75). Schema synchronization rides along: extract the shared `"required": [...envelope]`/property fragments in `handlers/schema/artifacts/*` into one `common` builder mirroring the base, so the mirror and the truth both become single-homed; extend the existing enum-drift-test style with a property-set equality check between `schema_for(kind)` output and a Rust-side fixture.

Must preserve: alias-in/snake-out bytes (model/tests/artifacts.rs:44-97), `GraphNode` response shape under the tagged union (cli_sdk_query_get.rs:22) including buffering tolerance for flatten-under-tag, deny error behaviors of the closed families, and all writer construction sites absorbing via `..envelope`.

Tradeoffs: strongest deduplication (type + fields + accessors + schema fragment); but collides head-on with the closed-record law on exactly the five most actively-tested structs, weakens missing-field error precision behind flatten, and requires an AssertionId decision. Wrong if: serde rejects the tag/flatten/buffering combination in practice for the query path, or error-message expectations are pinned harder than the evidence showed — either would fail loudly in existing suites, not silently.

**Candidate B — Macro-stamped field declarations plus generated trait impl (declaration consolidation, no storage move).**

Position: keep each struct's storage exactly as it is, but define a declarative macro (or a derive in the existing `provenance_macros` crate, which already provides `rule`/`verifies` infrastructures) that expands into the three field declarations with their exact current attributes and generates the envelope-trait impl per annotated struct.

Mechanism sketch: `record_envelope!` expands to `pub schema_version: SchemaVersion, pub scope_id: ScopeId, pub id: StableId` in place; serde still sees flat struct fields, so `deny_unknown_fields`, aliases, skip rules, field order, and internally-tagged decoding behave bit-identically — no serde hazard survives the expansion. The generated trait collapses `GraphNode` matches and version-guard plumbing. Construction sites optionally gain a macro helper for the leading triple, keeping writers one-token shorter without changing signatures. Schema synchronization then leans on testing rather than generation: add a per-kind equivalence test that feeds representative fixtures through both the hand-written JSON Schema (`jsonschema` crate is already compiled in, handlers/schema.rs:40) and the Rust struct via `serde_json`, asserting accept/reject agreement — generalizing the existing export-schema fixture discipline (tests.rs:307-415) to all seven kinds.

Must preserve: literally everything byte-wise; the risk surface is tooling, not wire.

Tradeoffs: zero wire or closure risk; mechanical and reversible; works uniformly across closed and open families; leaves three duplicated field lines visible at IDE level per struct and adds macro indirection (rust-analyzer expandability should be verified as part of the work). Wrong if: the bead's intent truly demands *one shared type object* rather than one shared declaration — B centralizes wording, not identity; also wrong if the team treats proc-macro expansion as an auditability regression relative to explicit fields.

**Candidate C — Trait-only consolidation (logic dedup; fields stay put).**

Position: introduce `trait RecordEnvelope` with accessors for the triple, implement it manually per family (small, dumb impls), and refactor only the generic consumers — `GraphNode` accessors, aggregate validators, CLI validators — onto it. No serde surface changes of any kind.

Mechanism sketch: the trait is the seam future mechanisms land behind; consumers stop knowing kinds individually while structs remain plain. Schema synchronization stays hand-written but gains the same fixture-equivalence test proposed in B (independent of mechanism choice, that test closes the schema gap discovered in finding 8).

Must preserve: trivially everything; this is the null-move baseline for the refactor half.

Tradeoffs: near-zero risk, zero dependency/tooling questions, genuinely shrinks the duplicated *logic* (per-kind match arms and per-kind schema-version call sites), but leaves ~35 lines of repeated field declarations; drift remains possible at the declaration site, and the bead's headline goal ("one shared typed base") is only partly served. Wrong if: reviewers expect the refactor to make adding an envelope-like fourth field impossible to forget — C offers no such forcing function.

**Candidate D — Newtype base nesting without flatten (explicitly offered for disposal).**

Position: `struct Source { pub base: RecordBase, /* rest */ }` with plain composition. This changes the wire shape (`{"base": {...}}` or a renamed inner object) unless paired with flatten, which collapses it back into Candidate A. Given the pins — shards are newline-delimited records with top-level stable `id` and `schema_version` (docs/state-format.md:3), model tests assert `source["schema_version"] == 1` at the top level (artifacts.rs:79), and the export schema fixes the top-level property list (schema/tests.rs:307-415) — D requires either a canonical-state migration and wire/protocol coordination (contradicting the ADR 0008 lineage of moving compatibility through `SDK_PROTOCOL_VERSION` increments only, docs/adr/0008:40-41) or degrades into A. Included so the reviewer sees the fourth corner of the design space rejected on evidence, not taste.

**Decisions left explicitly to the human reviewer:**

1. Mechanism choice among A/B/C (and formal burial of D), given the closure conflict documented in finding 3/finding 4.
2. For Candidate A specifically: may the five struct-level `deny_unknown_fields` ideation records (a) join the consolidation with closure re-homed to read-guard style, (b) stay excluded so consolidation is knowingly asymmetric, or (c) be dropped from denial — the latter presumably unacceptable?
3. Unify `AssertionId` into `StableId` (wire-transparent today) or carve a base that tolerates two id types — i.e., how literal is "one shared type"?
4. Should `graph_reference`'s plain-u32 `schema_version` fields (graph_reference.rs:30,61,69) be promoted to `SchemaVersion` as part of this bead, or left untouched to bound blast radius?
5. Schema-show synchronization: extension-test route (fixture equivalence via `jsonschema`), generation route following the `.github/release-targets.json` → `packages/provenance/scripts/package-engine.js` committed-artifact precedent (which would admit a new `schemars`-style dependency, currently absent from Cargo.lock), or both staged.
6. Confirm or reject the verdict that this is purely internal — repository evidence supports it (findings 5-7), but the reviewer should ratify "no `SDK_PROTOCOL_VERSION` bump, no migration, pins stand" before planning proceeds.
7. Whether the demonstrated staleness of two conversation-context anchors (TS-mirror shrinkage is mostly a non-event; generator lives at `packages/provenance/scripts/`, not `scripts/`) should update the epic description before child issues are cut.
