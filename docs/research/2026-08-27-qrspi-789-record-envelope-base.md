---
date: 2026-08-27
bead: provenance-789
epic: provenance-46p
stage: structure-complete-awaiting-disposal
model: glm-5.3-flash-high
---

provenance-789 — Consolidate the record envelope into one shared typed base (epic provenance-46p)

=== QUESTION ===

Every record repeats an envelope of `schema_version`, `scope_id`, and `id`. How should this envelope become one shared Rust type in `provenance-core` and in the store's writers? And how should the published per-kind JSON Schemas (`provenance schema show`) stay synchronized with that Rust type?

Sub-questions:

1. Which record families carry the envelope? Do their id types differ (`StableId`, `AssertionId`, plain `u32`) so much that one struct cannot fit all?
2. Can the refactor stay purely internal? It must not bump the canonical schema_version, must not increment the SDK `protocol_version` (currently 5), and must not need migration. Do the round-trip pins permit that?
3. A shared base must survive three serde hazards. (a) Some structs declare `#[serde(deny_unknown_fields)]` themselves (the entire ideation family). (b) The internally tagged union `GraphNode` uses `tag = "node_type"` beside record content. (c) Model tests pin camelCase input and snake_case output.
4. Do the candidates differ in what they consolidate? One goal moves field storage into a shared type. Another removes repeated accessors and validation. These are separate goals.
5. After the refactor, what stops the hand-written JSON Schemas from drifting away from the Rust structs? Options are regeneration (precedent: npm platform packaging), extended equivalence tests, or both.
6. Will the TypeScript mirrors under `packages/provenance/src/protocol.ts` shrink mechanically afterward? The conversation context asserts that they will.

In scope:

- Record structs carrying the envelope triple, under `crates/provenance-core/src/model/**`.
- Store-side construction sites in `crates/provenance-store/src/state_store/*writers*` and `graph_reference*`.
- `GraphNode` internals (`protocol/node.rs`) and all generic record consumers (`validate_source_schema_versions`, CLI write gates, `handlers/schema.rs`).
- The `schema show` schemas (`handlers/schema/artifacts/*`) and their drift tests (`handlers/schema/tests.rs`).
- Typed-spec wire pins (`rule_rust_typed_input_round_trip`), as far as they constrain mechanism choice.
- Whether the TS mirrors need changes at all.

Out of scope:

- Changing the canonical state format or the `SUPPORTED_SCHEMA_VERSION` value.
- Any `SDK_PROTOCOL_VERSION` change or new wire capability.
- Adding or removing record families; renaming wire fields; adding camelCase aliases to envelope fields.
- Deciding which artifacts the `Validate` command accepts, beyond current behavior.
- Writing code, plan documents, or child issues. This session stops here.

The answer must let a human reviewer decide four things. (i) Which mechanism fits the fail-closed laws: a flattened shared type, a macro-stamped declaration, or a trait only. (ii) Can struct-level `deny_unknown_fields` coexist with a shared flattened base? Or do those families stay out of the consolidation? (iii) Should `schema show` output become generated and committed, like the npm platform artifacts? (iv) Does this refactor carry any version or migration obligation?

=== RESEARCH ===

**1. Envelope repetition census.** Every stored record family declares the same triple as its first three fields. Examples follow. `Source` (artifacts.rs:224-227), `Requirement` (:285-287), `Resolution` (:356-358), `Rule` (:405-407). Then `Boundary`, `Topic`, `Question` (shaping.rs:113-116, 125-128, 142-145); `Domain` (services.rs:6-9); `Thread` and `Message` (collaboration.rs:43-47, 54-57); four integration records (integrations.rs:108-110, 133-135, 154-156, 173-175); `Edge` (graph.rs:77-81). The ideation records join them: `Contribution` (ideation/contributions.rs:66-69), `SynthesisPacket` (ideation/synthesis.rs:85-90), `ProposalCard` (ideation/proposals.rs:22-27), `DispositionRecord` (ideation/dispositions.rs:32-36), `AssertionRecord` (ideation/lifecycle.rs:38-43). `cli_record_schema_versions.rs:5-32` lists 17 stored families, including legacy dispositions. That count matches the repetition above.

**2. Id-type variance inside the "identical" triple.** Most families declare `pub id: StableId`. Two deviations exist. `AssertionRecord.id` is an `AssertionId`, a transparent newtype over `StableId` (lifecycle.rs:20-22, 40-43). The graph-reference export structs use `schema_version: u32` directly, not `SchemaVersion` (graph_reference.rs:30, 61, 69). One shared base cannot hold both id types. It needs type unification or a type parameter.

**3. Closure asymmetry — the load-bearing constraint.** The ideation family is closed at the model level. These structs declare `#[serde(deny_unknown_fields)]`: `Contribution` (contributions.rs:64-66), all synthesis payloads and `SynthesisPacket` (synthesis.rs:8-9 through 86-88), proposal inputs and cards (proposals.rs:10, 21-22), disposition records (dispositions.rs:7, 16, 23, 32), `AssertionRecord` (lifecycle.rs:38-40), plus `IdeationTarget` and `IdeationEvidenceReference` (ideation.rs:274-276, 283-285). The other graph records carry no such attribute. This applies to `Source`, `Requirement`, `Resolution`, `Rule`, shaping, collaboration, and integration records. Closure for those families lives elsewhere. `state_store/readers.rs:60-77` refuses unsupported-version rows for every family before serde sees them. The protocol-input layer is also closed: typed_spec.rs:19-107, query.rs:41-161 (8 structs), state_store/inputs.rs:214-257. So five-plus ideation structs face an attribute-level collision. The graph families do not.

**4. In-repo flatten precedent — including a documented limit.** `#[serde(flatten)]` appears in eight places. `QueryResponse<Result>` flattens its result generically (protocol/response.rs:16-23). Further sites: coverage.rs:174, statement_policy.rs:14, dictionary.rs:13, proposal_surfaces.rs:83, prime.rs:9, operations/plan.rs:22 and :29. plan.rs:16-19 states the house limit: "`TypedSpecResult` flattens ..., so `TypedSpecResult` must never gain `deny_unknown_fields`". Flatten therefore does not combine with deny on the flattened target today. No current struct puts `flatten` and `deny_unknown_fields` on itself together.

**5. Internally tagged union and generic-consumer duplication.** `GraphNode` is internally tagged: `#[serde(tag = "node_type", rename_all = "snake_case")]` over boxed full records (protocol/node.rs:16-25). Every variant field must deserialize correctly under serde's internal content buffering. Its accessors repeat kind matching today: `id()` (:39-48), `retired()` (:54-61), `searchable_text()` (:64-88). An envelope trait could replace these matches. Note also: the store guard reads `schema_version` from raw JSON, not from a struct. readers.rs:79-100 gives the reason: "the struct is what we refuse to build until the version is known". Rust-shape changes therefore leave this gate unchanged by construction.

**6. Version governance is already one-place; the protocol lineage supports "internal-only".** `SUPPORTED_SCHEMA_VERSION: SchemaVersion = SchemaVersion(1)` is declared once. A comment there says there is "no second copy of the number to update" (aggregate_validation.rs:13-19). `ensure_supported_schema_version` enforces it through `rule_schema_version_one` (:41-49). It runs at aggregate level for contributions, synthesis packets, and proposals (:117-129), for dispositions (:273), and for assertions (assertion_validation.rs:25). The CLI artifact validator calls the same functions (handlers/validate.rs:46-76). `cli_record_schema_versions.rs:36-72` proves v2 refusal across all 17 families. ADR 0008 set the pattern for earlier work: "The wire change increments the SDK protocol from 4 to 5. The canonical state schema stays at version 1" (docs/adr/0008:40-41). `SDK_PROTOCOL_VERSION = 5` stands at protocol.rs:25. docs/state-format.md:3,7 pins the shard shape ("stable string `id` fields, `schema_version`") at version 1. Earlier adoption-related wire work landed with "state schema version 1 needs no migration" (docs/state-format.md:17-18).

**7. The round-trip pins that decide sub-question 2.** Model tests pin byte behavior per family. Their names state what they pin. `enriched_source_and_requirement_records_roundtrip_without_schema_bump` decodes `"commitPin"` and `"effectiveDate"` as camelCase. It asserts snake_case output and an unchanged `schema_version` (model/tests/artifacts.rs:44-97). Retirement omission and explicitness have pins at :3-42. Similar fixture tests cover shaping, services, collaboration, and ideation (model/tests/shaping.rs:8-90, services.rs:5-28, ideation.rs:11-150). The typed-spec input pin is stricter. Under `#[verifies("rule_rust_typed_input_round_trip", examples)]`, authoring/tests.rs:301-314 checks serialize→decode→serialize equality against `TypedSpecInput`. That struct declares `deny_unknown_fields` (typed_spec.rs:17-19). Two context corrections apply here. First: `TypedSpecInput.schema_version` is a plain `u32 = 1`, and the struct carries no `scope_id` or `id` (typed_spec.rs:20-31). It is not itself an envelope carrier. Second: query responses wrap records in an encode-only flatten envelope keyed by `protocol_version` and `operation` (response.rs:11-23). Tests assert its bytes, for example cli_sdk_query_get.rs:22 (`answer["node"]["node_type"]`). Conclusion: keep field names, positions, aliases, defaults, skip rules, and deny semantics unchanged, and none of these pins breaks. No pin depends on how many source-line repetitions produce the fields.

**8. Published JSON Schemas are hand-written mirrors with partial drift tests.** `schema show` dispatches per kind to hand-written `json!({...})` builders. Example: the contribution schema hardcodes required `["schema_version","scope_id","id",...]` and sets `"schema_version": {"const": 1}` (handlers/schema/artifacts/contribution.rs:5-45). schema_for wraps them (handlers/schema.rs:19-36). Drift protection covers enums only today. Exhaustive variant-array checks sit at handlers/schema/tests.rs:140-169. Enum-value comparisons against model serialization sit at :172-264. The graph-reference-export schema goes further. It validates complete record fixtures, envelope included (tests.rs:307-342). It rejects `"schema_version": 2` and an unexpected `origin_thread` (:368-415). No test anywhere compares a schema property-name set against its matching Rust struct. `jsonschema::JSONSchema` already compiles and validates here (handlers/schema.rs:40-46). Fixture-driven equivalence tests need no new tooling.

**9. House generation precedent.** The npm platform packaging shows the accepted pattern. One JSON source of truth lives at `.github/release-targets.json`; release.yml:95 passes it as `--targets`. A generator consumes it: `packages/provenance/scripts/package-engine.js:4-31` reads the targets file and writes committed platform manifests. Release contract tests guard the relationship (`.github/scripts/release-contract.test.py`). Note: the context anchor gave a wrong generator path. No `scripts/package-engine.js` exists at the repo root.

**10. Contradicting evidence found while checking context claims.**
(a) The TypeScript shrinkage claim is largely false. `packages/provenance/src/protocol.ts` never spells out the per-record envelope. Its `GraphNode` is `{ node_type; id; retired?; [field: string]: unknown }` (protocol.ts:197-203). Its `TypedSpecDocument` carries only `schema_version: 1` (:59-67). Envelope consolidation thus needs zero TS edits, at most.
(b) None of the three envelope fields has a camelCase alias anywhere. Searches for `alias = "scopeId"/"schemaVersion"/"nodeId"` return nothing. Aliases exist only on non-envelope fields.
(c) Production writers build records with exhaustive struct literals, triple first (rule_writers.rs:40-59; ideation_writers.rs:49-50 uses `SUPPORTED_SCHEMA_VERSION`; thread_writers.rs:45,69; verification_bindings.rs:51; graph_reference.rs:195-242). A move-to-base refactor touches every writer, unless struct-update syntax absorbs the change.

Evidence-split checklist —

Repository facts (each cited above): the repetition across ~17 families; the `AssertionId` and plain-u32 variance; ideation closure at struct level versus graph-family openness; the plan.rs:16-19 note on flatten and deny; the raw-JSON version guard; the one-place `SUPPORTED_SCHEMA_VERSION` and its call sites; ADR 0008's split between wire and canonical versions; the per-model-file round-trip pins; hand-written schema builders with enum-only drift tests; the npm packaging generation pattern; the TS mirror index-signature shape; the absence of envelope-field aliases; the writer construction sites.

My inference (the repository alone cannot prove these): serde derive rejects `deny_unknown_fields` together with `flatten` on one struct. This is a known upstream limit; plan.rs:16-19 reflects it indirectly, but nothing here triggered it. Flatten forwards serialized fields in declaration order. So a flattened base in first position keeps today's JSON key order. Internally tagged deserialization buffers content; these self-describing types tolerate that. Error text for missing envelope fields would change behind flatten, and humans read those validator diagnostics. Each inference needs a compile-time spike first. None of them changes the candidates below.

=== STRUCTURE ===

**Candidate A — Shared `RecordEnvelope` struct, flattened first member, accessor trait (full type consolidation).**

Position: define one owned type, `RecordEnvelope { schema_version: SchemaVersion, scope_id: ScopeId, id: StableId }`. Every graph-family record replaces its first three fields with `#[serde(flatten)] pub envelope: RecordEnvelope`, declared first. A small trait `EnvelopeRef { fn envelope(&self) -> &RecordEnvelope }` exposes the triple. Cross-kind consumers match through it: `GraphNode::id/retired/searchable_text` (node.rs:27-88), `validate_source_schema_versions` (aggregate_validation.rs:117-135), and the CLI gates. For ideation families, only id typing blocks literal sharing. Unify `AssertionId` toward `StableId`; it is transparent anyway (lifecycle.rs:20-22). Or exclude those five structs.

Mechanism sketch (interface and invariant level): the base owns only the three fields and a constructor taking validated ids. ids.rs:14-17 keeps ids constructible only through `new`. Serde derives stay on the outer structs. Flatten preserves the flattened struct's field names, and first position preserves today's `schema_version, scope_id, id` key order. Deny-free graph families adopt the base fully. Ideation families cannot combine their struct-level `deny_unknown_fields` with flatten (plan.rs:16-19 documents the coupling; compile-time rejection is believed). Candidate A then offers three choices. (a) Apply it to non-closed families only. (b) Drop deny from those structs; unacceptable, because unknown fields would then pass silently instead of failing closed. (c) Re-home ideation closure onto pre-read inspection, readers.rs-style; this is a real design fork, because validate.rs surfaces serde messages directly today (handlers/validate.rs:47-75). Schema synchronization follows the same move. Extract the shared required-fields and property fragments in `handlers/schema/artifacts/*` into one `common` builder that mirrors the base. Then extend the existing enum-drift test style. Compare the property set of `schema_for(kind)` output against a Rust-side fixture.

Must preserve: the alias-in/snake-out bytes (model/tests/artifacts.rs:44-97); the `GraphNode` response shape under the tagged union (cli_sdk_query_get.rs:22), including tolerance for flatten under the tag; deny error behavior of the closed families; writer construction sites absorbed through `..envelope`.

Tradeoffs: strongest consolidation of type, fields, accessors, and schema fragment. But it conflicts directly with the closed-record law on the five most-tested structs. Missing-field errors lose precision behind flatten. An AssertionId decision becomes necessary. Failure modes: serde may reject the tag/flatten/buffering combination on the query path. Or error-message expectations may be pinned harder than the evidence showed. Both failures appear loudly in existing suites, not silently.

**Candidate B — Macro-stamped field declarations plus generated trait impl (declaration consolidation, no storage move).**

Position: keep each struct's storage as it is. Define a declarative macro, or a derive inside the existing `provenance_macros` crate; that crate already provides the `rule` and `verifies` infrastructure. The macro expands the three field declarations with their exact current attributes. It also generates the envelope-trait impl per annotated struct.

Mechanism sketch: `record_envelope!` expands to `pub schema_version: SchemaVersion, pub scope_id: ScopeId, pub id: StableId` in place. Serde still sees flat struct fields. So `deny_unknown_fields`, aliases, skip rules, field order, and internally tagged decoding stay identical, and no serde hazard survives the expansion. The generated trait collapses the `GraphNode` matches and version-guard plumbing. Construction sites can gain a macro helper for the leading triple; writers get shorter without signature changes. Schema synchronization leans on testing, not generation. Add a per-kind equivalence test: feed representative fixtures through both the hand-written JSON Schema (`jsonschema` crate is already compiled in, handlers/schema.rs:40) and the Rust struct through `serde_json`, and assert equal accept/reject verdicts. This generalizes the export-schema fixture discipline (tests.rs:307-415) to all seven kinds.

Must preserve: everything stays identical at byte level. The risk sits in tooling, not on the wire.

Tradeoffs: zero wire risk and zero closure risk. Mechanical and reversible. Works the same way for closed and open families. Costs: three field lines remain visible per struct, plus macro indirection; verify rust-analyzer expandability during the work. Wrong if: the bead truly demands one shared type object rather than one shared declaration. B centralizes wording, not identity. Also wrong if macro expansion hurts auditability relative to explicit fields.

**Candidate C — Trait-only consolidation (logic deduplication; fields stay put).**

Position: introduce a trait `RecordEnvelope` with accessors for the triple. Implement it manually per family with small impls. Refactor only the generic consumers onto it: `GraphNode` accessors, aggregate validators, CLI validators. Serde behavior does not change at all.

Mechanism sketch: the trait is the seam where later mechanisms attach. Consumers stop matching kinds individually, while structs stay plain. Schema synchronization stays hand-written but gains the fixture-equivalence test proposed in Candidate B. Independent of mechanism choice, that test closes the schema gap from finding 8.

Must preserve: trivially everything. C is the smallest possible step for the refactor half.

Tradeoffs: near-zero risk, no dependency questions, no tooling questions. It genuinely shrinks repeated logic: per-kind match arms and per-kind schema-version call sites. But ~35 lines of field declarations remain. Declaration-site drift stays possible. The bead headline asks for "one shared typed base"; C serves that goal only partly. Wrong if: reviewers expect a forcing function that makes forgetting a future fourth envelope field impossible. C offers none.

**Candidate D — Newtype base nesting without flatten (offered explicitly for disposal).**

Position: nest a base plainly: `struct Source { pub base: RecordBase, /* rest */ }`. Plain composition changes the wire shape (`{"base": {...}}` or a renamed inner object). Pairing it with flatten collapses D back into Candidate A. The pins forbid the unmigrated form. Shards hold top-level stable `id` and `schema_version` (docs/state-format.md:3). Model tests assert top-level values like `source["schema_version"] == 1` (artifacts.rs:79). The export schema fixes the top-level property list (schema/tests.rs:307-415). D therefore needs a canonical-state migration plus wire coordination. That contradicts the ADR 0008 lineage, which moves compatibility through `SDK_PROTOCOL_VERSION` increments only (docs/adr/0008:40-41). D appears here so the reviewer sees the fourth option rejected on evidence, not on taste.

**Decisions left explicitly to the human reviewer:**

1. Choose the mechanism among A, B, and C, and formally bury D. The closure conflict in findings 3 and 4 drives the choice.
2. Candidate A only: choose how the five struct-level `deny_unknown_fields` ideation records proceed. (a) Join with closure re-homed to read-guard style. (b) Stay excluded, making the consolidation knowingly asymmetric. (c) Drop denial; presumably unacceptable.
3. Unify `AssertionId` into `StableId`; it is wire-transparent today. Or carve a base holding two id types. How literal is "one shared type"?
4. Promote the plain-u32 `schema_version` fields in `graph_reference` (graph_reference.rs:30,61,69) to `SchemaVersion` in this bead? Or leave them untouched to bound the blast radius?
5. Choose the schema-show synchronization route. Route one: extension tests with fixture equivalence through `jsonschema`. Route two: generation following the `.github/release-targets.json` → `packages/provenance/scripts/package-engine.js` committed-artifact precedent; it admits a new `schemars`-style dependency, absent from Cargo.lock today. Route three: both, staged.
6. Confirm or reject the pure-internals verdict. Findings 5-7 support it. Before planning starts, ratify: no `SDK_PROTOCOL_VERSION` bump, no migration, pins stand.
7. Two conversation-context anchors proved stale. TypeScript mirror shrinkage is mostly a non-event. The generator lives at `packages/provenance/scripts/`, not `scripts/`. Should the epic description be updated before child issues are cut?
