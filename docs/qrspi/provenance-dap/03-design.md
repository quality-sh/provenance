# Design: A pure authoring kernel in provenance-core, one stateful seam in provenance-store

- type: design-discussion (QRSPI dogfood corrective run)
- revision: 7 (created from revision 6 by folding repair texts R6-A and R6-B exactly
  as refined by CO-R6-1 and CO-R6-2, per Ben's gate authorization; revision 6 and its
  report are immutable superseded history and attach to nothing)
- producer: dogfood producer (this orchestrating session; identity distinct from both
  reviewer agents and the synthesis agent)
- status at authoring time: awaiting fresh semantic-ownership review of revision 7

## Revision 7 response (repair-folding record)

| Repair | Where it landed in this revision |
|---|---|
| CO-R6-1 / R6-A with refinements R1 and R2 (fixes MUST-FIX O-R6-1; subsumes O-R6-2) | DR1 point 2 rewritten: (i) empty-statement text identified as the ONLY TS-only structural check AMONG THE PARTITIONED FAMILIES, with the other requireText guards (source name, document reference, requirement description) disclosed as additional build()-only TS-parity content checks that never gate the wire (R1); (ii) the engine-side wire-gating set re-enumerated to include the declaration_ids structural ensures (identity.rs:157-158) with the id-resolution carve-out, and validate_references (identity.rs:245-270) as a document-decidable kernel-homed check; the closing sentence now reads "Only the engine-side check set of DR2 gates the wire" (resolving O-R6-2). DR2's relocated cell re-enumerated identically, with the kernel calls PINNED at the existing pipeline points, preserving today's rejection precedence (R2), and the invariant restated over accept/reject identity with V2 pinning the error classes. V2 leg (ii) gains duplicate-source/requirement-key and undeclared-reference rejection cases. The ownership map's store row names the two added families. |
| CO-R6-2 / R6-B with the DR6-narrowing refinement (fixes O-R6-3) | The disclosure row below accounts for every non-amendment rev-5→rev-6 addition, including the DR6 "(store/core)"→"(store)" narrowing. |
| Non-amendment disclosure (CO-R6-2) | Carried from revision 6, disclosed: TypedSpecDiagnostic's forced store placement rationale (embeds ste100 types core does not depend on); DR10 "walkdir included"; the verified blast-radius figure (11 files, none outside crates/); the sixteen-operation count; DR11(b) runner homes; the full-union-at-wire alternatives-rejected entry; G2's two-readings phrasing; the DR6 "(store/core)"→"(store)" derive-target narrowing (correct: EngineInfo already derives Deserialize, core protocol.rs:35). New in revision 7, same disclosure discipline: the normalize_rule_relationships citation corrected from :120-143 to :112-143 (the function's true start; all named semantics live in both ranges — the synthesis-noted citation hair). None of these alters architecture or scope. |

## Acceptance scope (dogfood adaptation of DesignAcceptanceScope)

- Question (ticket-equivalent): provide a native Rust requirements-as-code authoring
  surface for Provenance with parity to the TypeScript SDK, while the Rust engine
  remains sole owner of validation, identity, planning, and state mutation.
- Ordered sources: (1) QUESTION artifact; (2) RESEARCH artifact (F1..F12, U1..U4,
  proposed D1..D13); (3) Ben's Design inputs I1/I2; (4) gate correction B1-B7; (5)
  gate correction C1-C8; (6) Ben's revision-6 authorization with CO-R5-1..8; (7) Ben's
  revision-7 authorization with CO-R6-1/CO-R6-2 (2026-08-23). Prior STRUCTURE output:
  diagnostic only.
- Repository pins: provenance checkout @ 11ebe84cbbf343b8cf37766340346940bdb706d6
  (main); workflowd checkout @ a80014dcc1ce38195c8bc8c0e093c159d76cd731. Unprefixed
  paths are provenance-repo-relative.
- Policy pins (dogfood): design policy = docs/qrspi-contract.md §Design Acceptance
  Subflow as bundled in workflowd:skills/qrspi-design-structure; automated revision
  budget three (exhausted at revision 3); revisions 4-7 are each explicitly
  human-authorized. Promotion policy = PROHIBITED-THIS-RUN (G5). Structure policy = bd
  memory qrspi-structure-split-flow.

## Decision register

Status legend: RESOLVED-BY-EVIDENCE / HUMAN-GATE as before.

### DR1 — The pure authoring kernel: `provenance_core::authoring` (RESOLVED-BY-EVIDENCE, given C1-C3)

A new module family in the EXISTING provenance-core crate (module split under the
500-line cap). Pure: no I/O, no env access, no clocks, no repository state, no
provenance-store or provenance-scanner types. It owns four things:

1. Immutable construction — by-value builders that consume `self` and return frozen
   values (the Rust-native form of the TS frozen-instance discipline,
   fluent-spec.ts:42), over arbitrary string keys.
2. Language-neutral structural validation at `build()` — the UNION of two check sets,
   stated precisely (CO-R6-1):
   (i) TS-only structural checks, for which the kernel becomes a NEW single home,
   applied at `build()` and NEVER at the wire: empty statement text — the ONLY TS-only
   check among the duplicate-key/reference/statement families partitioned here
   (`requireText`, bound-declarations.ts:282-283; statement call sites
   bound-materialize.ts:38 and :110). Disclosed for inventory completeness (R1): TS's
   `requireText` also guards source name (bound-declarations.ts:106), document
   reference (:92), and requirement description (:220); these are additional
   build()-only TS-parity content checks with no engine counterpart today, and they
   likewise never gate the wire.
   (ii) today's engine-side structural checks, which relocate into the kernel
   unchanged and continue to gate the wire: empty rule keys, duplicate addresses, and
   relationship collisions (typed_specs/identity.rs:35-51); empty and duplicate
   source/requirement keys — the structural ensures inside `declaration_ids`
   (identity.rs:157-158) — with the ID-RESOLUTION CARVE-OUT: the structural ensures
   relocate, while id resolution itself (address→id mapping via `resolve_id`,
   migration, explicit-id validation, and the resolution-dependent id-collision
   ensures at identity.rs:168-172 and :65-69) remains store-owned per C4; and
   undeclared source/requirement reference validation (`validate_references`,
   identity.rs:245-270) — document-decidable (it reads only the key sets of the
   document's own declared sources/requirements, never repository state), and
   therefore kernel-homed under C2, while all stateful identity and reference
   authority remains in provenance-store; plus malformed explicit ids via the
   existing core `rule_id_charset` predicate (ids.rs:18-24).
   Only the engine-side check set of DR2 gates the wire.
3. Declaration-address construction — the four legal shapes, single-homed beside the
   existing `DeclarationAddress` type (core integrations.rs:53-80):
   `[spec,"source",key]`, `[spec,"requirement",key]`,
   `[spec,"requirement",req,"rule",key]` (one owner), `[spec,"rule",key]` (shared).
   Ownership and call direction, stated once and normatively: the KERNEL owns the
   shape constructors and the structural checks; the STORE calls into the kernel at
   ingestion (store → kernel call direction); the store's current copies
   (identity.rs:9-15 source/requirement shapes; rule_addresses.rs:70-88 rule shapes
   and :7-35 inference/validation; identity.rs:35-51 structure; the declaration_ids
   structural ensures at :157-158; validate_references at :245-270) become those
   kernel calls. No third implementation exists anywhere; V2's kernel-equivalence
   suite guards the transition.
4. Canonical assembly — deterministic, locale-free ordering (sources/requirements by
   key, rules by serialized address, lexicographic UTF-8 byte order) and
   materialization into the existing desired-state document. Canonical ordering
   applies ONLY to kernel-authored assembly (`SpecDocument::materialize()`); it is
   never applied to wire-received documents (DR2).

Representative API sketch (concrete calls; signatures indicative, names final at Plan):

    use provenance_core::authoring::{spec, source, requirement, rule};

    let doc = spec("share-links")
        .requirements([
            requirement("sharing")
                .statement("Users can securely share documentation")
                .from(source("sharing-policy").document("docs/sharing-policy.md"))
                .rules([
                    rule("expiry")
                        .statement("Share links must expire within 30 days")
                        .implemented_at("src/share_links.rs", "create_share_link"),
                    rule("audit").statement("Share-link access is audited")
                        .id("rule_share_link_audit"),          // explicit-id escape hatch
                ]),
        ])
        .build()?;                       // pure: structural checks, addresses, canonical order
                                         // Err(AuthoringError) carries every violation, TS-parity wording

    let handles = doc.handles();         // typed, address-bearing handles
    let expiry = handles.requirement("sharing")?.rule("expiry")?;   // -> RuleHandle { address: [...] }

    let input: TypedSpecInput = doc.materialize("spec://rust");     // C3: the existing store document

    // stateful seam (I1, in-process; unchanged owners, C4):
    let plan   = provenance_store::operations::plan(None, &scope, input.clone())?;  // -> TypedSpecPlan
    let result = provenance_store::operations::apply(None, &scope, input)?;         // -> TypedSpecResult

`SpecDocument` is frozen and canonical; `materialize(declared_by)` is total (no I/O,
no failure beyond what `build()` already rejected). Typed-handle projection with
compile-time key safety stays a FRONTEND concern (C6): the kernel's `handles()` is
string-keyed and address-bearing; Rust type-state/macro sugar wraps it (DR5), TS's
literal-key unions wrap its wire equivalent.

`TypedSpecInput`, `TypedSourceInput`, `TypedRequirementInput`, `TypedRuleInput`,
`TypedImplementationInput` RELOCATE from crates/provenance-store/src/state_store/
inputs.rs:137-205 to provenance-core (they are wire/protocol types; core is the
protocol home — EngineInfo, queries, SDK_PROTOCOL_VERSION already live there,
protocol.rs). provenance-store re-exports them at the current paths (store already
depends on core; nothing else changes for existing consumers). Core's dependency set
is UNCHANGED — the family needs serde and camino only, both already core dependencies
(verified). Result types (`TypedSpecResult`, `ReconciledResource`, diagnostics) STAY
store-owned: they are reconciliation outputs, not authoring inputs (and
`TypedSpecDiagnostic` embeds ste100 types core does not depend on, forcing the
placement). The remaining wire-shaped inputs in inputs.rs — `BeginVerificationInput`
(:276-292), `DeclarationReferenceInput` (:312-317), `CompleteVerificationInput`
(:319-326) — STAY store-owned beside the DR4(b) verification operations they
parameterize (operation inputs live beside their operations); the inputs.rs partition
is thereby total.

### DR2 — The wire: zero format change; ingestion re-specified as kernel materialization with wire acceptance unchanged (RESOLVED-BY-EVIDENCE, given C6; CO-R6-1 folded)

The versioned transport for non-Rust frontends is the EXISTING `sdk plan` / `sdk
apply` operations at protocol version 4, schema_version 1 — no new operation, no new
version, no wire-shape change. What changes is WHERE the semantics live, under an
exhaustively partitioned check discipline:

- RELOCATED TO THE KERNEL, unchanged, and applied at wire ingestion — exactly today's
  engine-side check set: the structural checks at typed_specs/identity.rs:35-51
  (empty rule keys, duplicate addresses, relationship collisions); the structural
  ensures inside `declaration_ids` (identity.rs:157-158 — empty and duplicate
  source/requirement keys), with id resolution itself (resolve_id, migration,
  explicit-id validation, and the resolution-dependent id-collision ensures at
  :168-172 and :65-69) remaining store-owned per C4; undeclared source/requirement
  reference validation (`validate_references`, identity.rs:245-270 —
  document-decidable, kernel-homed); relationship normalization
  (`normalize_rule_relationships`, identity.rs:112-143, including the legacy singular
  `requirement` merge, empty/repeat rejection, and its BTreeSet ordering); and
  address inference/validation (rule_addresses.rs:7-35).
- RETAINED AT THE STORE SEAM, unchanged: the envelope checks in `validate_typed_spec`
  (typed_specs.rs:315-340 — schema_version, declared_by, spec, scope, including its
  repository-state-dependent scope-existence leg), and everything on C4's reserved
  list (id resolution, ownership, STE gates, locking, mutation).
- NOT ADDED TO THE WIRE: the TS-only checks of DR1 point 2(i) — empty-statement text
  and the disclosed requireText content guards — apply at kernel `build()` only, for
  kernel authors. WIRE ACCEPTANCE IS UNCHANGED: a document accepted or rejected by
  today's engine is accepted or rejected identically after this design.
- PINNED CALL PLACEMENT (R2): the store's kernel calls sit at the EXISTING pipeline
  points — the current check sites inside `desired_typed_ids` (typed_specs.rs:63,
  :70, :83) and `reconcile_typed_spec` (:126) — so today's rejection precedence for
  multi-defect documents is preserved, not merely accept/reject identity. The
  wire-acceptance-identity invariant binds over accept/reject identity and, through
  V2's pinned error classes (leg (ii)), over the rejection classes.
- NO REORDERING: ingestion does not reorder anything. The response envelope
  (`TypedSpecResult.resources` and the diagnostics derived from it) is assembled in
  decoded wire order exactly as today (typed_specs.rs:145-190). Kernel canonical
  ordering applies only to kernel-authored assembly (`SpecDocument::materialize()`),
  never to wire-received documents. V2 includes a deliberately non-canonically-ordered
  fixture (mixed-case/non-ASCII keys) asserting response-order preservation.

Consequence, stated as normative: `address` fields, document ordering, and frontend
pre-validation are OPTIONAL wire content. A frontend that sends only raw declarations
is fully served; a frontend that computes them (today's TS) sends a valid superset and
loses nothing. The minimal language-neutral wire document a TS/future SDK sends:

    { "schema_version": 1,
      "spec": "share-links",
      "declared_by": "spec://typescript",
      "sources": [
        { "key": "sharing-policy", "name": "Sharing policy", "kind": "document",
          "reference": "docs/sharing-policy.md" } ],
      "requirements": [
        { "key": "sharing", "statement": "Users can securely share documentation",
          "sources": ["sharing-policy"] } ],
      "rules": [
        { "key": "expiry", "requirements": ["sharing"],
          "statement": "Share links must expire within 30 days",
          "implementation": { "file": "src/share-links.ts", "symbol": "createShareLink" } } ] }

(No `address`, no ordering obligation, `id` only as the explicit escape hatch — every
field above already exists in today's schema, verified field-for-field; today's TS SDK
documents remain valid unchanged.) Protocol governance is unchanged:
SDK_PROTOCOL_VERSION stays single-homed in core; the TS handshake and
`ensure_protocol_version` enforcement are untouched; Rust in-process compatibility is
cargo semver on core/store (the G2 wording items carry unchanged).

### DR3 — TypeScript responsibility reconciliation (RESOLVED-BY-EVIDENCE, given C6; the exact split)

MOVES TO THE CORE KERNEL (single normative home; the TS copies become
redundant-but-harmless fast-fail mirrors the moment kernel ingestion lands, and may be
deleted from the TS SDK in a later, separately-authorized cleanup with zero behavior
change):

| TS responsibility today | Evidence | Kernel home |
|---|---|---|
| Structural validation (statement/content text checks; duplicate keys; undeclared refs) | packages/provenance/src/bound-materialize.ts:29-44; spec.ts:102-146; engine twins identity.rs:35-51, :157-158, :245-270 | kernel `build()` (full union set of DR1 point 2); wire ingestion applies the ENGINE SUBSET only (DR2's partition, pinned at the existing pipeline points) |
| Declaration-address construction (four shapes; local-vs-shared by owner count) | bound-declarations.ts:143,157; fluent-spec.ts:445-453; engine twin rule_addresses.rs | kernel address constructors (store calls into them, DR1 point 3) |
| Canonical document assembly/ordering | bound-materialize.ts:147-151,161-163 (ICU localeCompare — environment-sensitive) | kernel canonicalization (byte order), applied ONLY to kernel-authored materialize(); wire-received documents are never reordered (DR2). |

REMAINS FRONTEND-SPECIFIC (host-language ergonomics, per C6):

| Frontend | Keeps |
|---|---|
| TypeScript | Literal-key unions and typed handle projection; invariant `in out SpecKey` phantom affinity (bound-types.ts:57-77); compiler-API `implementedBy` symbol lookup (implementation-reference.ts); verification callback execution + error serialization (index.ts:407-451); engine-path resolution, npm packaging, runtime handshake (engine.ts). |
| Rust | Type-state/macro projection (`provenance_spec!` with the const-assert identifier↔key link, `implemented_by!` — macro_rules! forms); verification callback execution under `catch_unwind` with the record-then-propagate recipe (DR8); settings/env; cargo packaging. |
| Every frontend | Nothing semantic: no id derivation, no reconciliation, no materialization of its own (C5) — mirrors only. |

### DR4 — The stateful seam, unchanged owners plus in-process reachability (C4 RESOLVED)

(a) Untouched: `StateStore::plan_typed_spec` and `StateStore::apply_typed_spec` remain
the sole owners of id resolution, migration, reconciliation, ownership refusal, STE
write gates, locking (`with_repository_publication`), and mutation
(typed_specs.rs:98-140; publication.rs:43-57). Kernel materialization feeds them;
nothing bypasses them; plan/apply keep their one shared code path.

(b) In-process reachability (required by C8/I2): the read-side operations that today
exist only inside the binary-only provenance-cli — the eight queries, verification
begin/complete and listings, plan enrichment (TypedSpecPlan = TypedSpecResult +
affected_rules + scanner evidence), path normalization, discovery, info — join
provenance-store as public operation functions, with the single new acyclic Cargo edge
store → scanner (scanner depends only on core+macros, verified) and
`CheckStatementRequest` relocating to core::protocol beside the statement operation
`provenance_ste100::check_descriptive` (already public). Discovery: the unified
store-side discovery operation adopts `resolve_repository`'s semantics
BYTE-IDENTICALLY — explicit-override precedence, canonicalization, and the no-repo
fallback to the canonicalized start directory (cli handlers/sdk.rs:145-171).
`layout::locate_repo_root` (layout.rs:80-89) is retained unchanged as public store
API; it has ZERO callers at this HEAD (synthesis-verified), so nothing depends on its
bail behavior and no caller's semantics change. `TypedSpecPlan` becomes store-owned.
`EngineInfo.engine_version` becomes provenance-store's CARGO_PKG_VERSION (G1 audit
item). Dispatch and human rendering stay in provenance-cli as thin adapters over these
functions plus the kernel ingestion (DR2). Alternatives to (b) were examined and
rejected in revision 4 (cli library target; scanner-homed ops; SDK-homed ops) and
those rejections stand.

### DR5 — Rust consumer shape, derived after the API (C7; determination RESOLVED-BY-EVIDENCE, name HUMAN-GATE G4)

Unchanged: direct consumption of provenance-core (authoring) plus provenance-store
(stateful seam) is fully supported. A THIN facade crate (working name
`provenance-sdk`) is warranted for exactly three frontend-ergonomic residues that
cannot live in the pure core: (1) verification orchestration — `verify(key, closure)`
composing store's begin/complete with `catch_unwind`, the record-then-propagate
recipe, and `#[track_caller]` call-site capture (DR8); (2) the macro projection
(`provenance_spec!`, `implemented_by!`); (3) settings/env parity (PROVENANCE_*
vocabulary, index.ts:453-461). The facade is re-exports plus these three — NO
materialization, NO validation, NO address logic, NO semantic layer (C5/C7). It wraps
the twelve TS-wrapped operations plus the declared `engine_info()` addition
(gate-visible).

### DR6 — Serialization contract (carried; re-scoped to the relocation)

Derives land with their owners: Serialize + `skip_serializing_if = "Option::is_none"`
on the input family (now in core) for fixture emission; Deserialize on
`TypedSpecResult`/`TypedSpecPlan`/query responses (store) for expected-outcome
decoding (store-only: EngineInfo already derives Deserialize, core protocol.rs:35).
Round-trip guard: result-side decoding must round-trip CLI-emitted JSON preserving
today's omissions (inputs.rs:269-273). Cross-frontend equivalence is asserted at
KERNEL OUTPUT (same raw declarations → same canonical TypedSpecInput → same store
outcome) — collation-independent. The flatten constraint stands, recorded at the
constrained type: `TypedSpecPlan` flattens `TypedSpecResult` (plan.rs:19-20), so
`TypedSpecResult` must never gain `deny_unknown_fields` (doc comment at the type).

### DR7 — Identity discipline (unchanged owners)

Frontends and the kernel construct addresses; ONLY the store resolves ids
(identity.rs:178-214), migrates local↔shared (identity.rs:75-110), enforces ownership
(identity.rs:245-291's ownership half — `validate_ownership` :272-291), and applies
the resolution-dependent id-collision ensures (:168-172, :65-69). The explicit-id
escape hatch on rules carries (bound-types.ts:25 parity). Nothing in this revision
moves id semantics.

### DR8 — Implementation references and verification (carried unchanged)

Baseline `.implemented_at(file, symbol)` on the existing required-file contract
(inputs.rs:201-205); `implemented_by!` as macro_rules! with compile-checked path
existence; the severable engine-side symbol→file extension remains HUMAN-GATE G6
(store-homed if accepted). Verification: `verify(key, closure)` with
`FnOnce() -> Result<(), E>` under `catch_unwind(AssertUnwindSafe(..))`; Err/unwind →
complete `failed` with serialized payload, THEN propagate (TS catch-record-rethrow,
index.ts:407-437); completion failure subordinate to closure failure (index.ts:428-431
parity); `panic = "abort"` documented limitation; sync-first with async as the G7
gate-visible capability delta; V11 pins both languages.

### DR9 — Settings and discovery (carried)

Settings vocabulary and env parity unchanged (index.ts:453-461; owners `spec://rust` /
`ci://rust`); dev override = cargo path/[patch] (G2 wording); discovery engine-side in
store per DR4(b) with preserved semantics; process-global CWD limitation documented.

### DR10 — Distribution and publication (HUMAN-GATE G1)

Unchanged: publication chain = the existing five crates plus the thin facade; audit
scope includes core's kernel + relocated input family (the ~130 re-export audit now
also covers the kernel's API), store's widened operation surface and scanner-inclusive
closure (walkdir included), engine_version identity at store (cli_sdk_info.rs:49
pin), `TypedSpecPlan` wire/semver coupling within store's API, and the absent MSRV.
Go/no-go, timing, audit scope: Ben's. No publication step in this design's
implementation.

### DR11 — Conformance proof (simplified by the kernel)

(a) Kernel-equivalence suite: identical raw declarations through (i) the Rust kernel
in-process, (ii) the CLI wire (today's TS-shaped documents, with and without optional
addresses), and (iii) documents with frontend-computed extras, all yield the SAME
canonical TypedSpecInput and the SAME store outcome. Homed beside the kernel (core
unit tests for canonicalization; provenance-cli tests for the wire path, which also
keep the byte-identical-CLI guarantee). (b) Cross-SDK corpus: identity lifecycle,
mixed-case keys, failing verification; fixtures engine-owned; runners per package (TS
= additive tests in packages/provenance; Rust = provenance-sdk tests; CLI =
provenance-cli tests); comparison keyed by address/stable id or end state. (c) Parity
ledger: owned by the facade crate, three delta classes; the ordering-delta entry
remains CLOSED as dissolved.

### DR12 — Example and CI (carried)

`examples/rust-sdk` mirroring `examples/typescript-sdk`; CI job mirroring the
`typescript-sdk` job's end-state assertions.

### DR13 — Prohibitions as design invariants

Exactly one materialization implementation (the kernel, C5); frontends' structural
checks are mirror-only (fast-fail what the kernel rejects; never accept what it
refuses); the kernel is pure (no I/O, env, state, store/scanner deps, C2/C3); no id
derivation outside the store; no write path around `with_repository_publication`; no
second protocol-version constant; no new semantic layer between kernel and store (C7);
typed_specs internals keep their restricted visibility.

## Carried obligations (CO-R5-9..11; no design-text semantics change)

- CO-R5-9: before any Plan stage relies on the transport rationale, re-verify cargo
  artifact-dependencies status (U3) from primary sources and record the finding.
  Owner: whoever runs Plan (Plan itself requires Ben's authorization).
- CO-R5-10: verify the const-assert assumption (const-context string comparison and
  assertions for the provenance_spec! identifier↔key link are standard stable Rust)
  before Plan relies on the macro projection. Owner: whoever runs Plan.
- CO-R5-11: V9's golden pins land BEFORE the DR4(b) relocation and stay green after;
  V10's binding-integrity scan runs post-relocation with workspace-wide scope (both
  explicit in V9/V10 below).

## What we're not doing

No provenance-engine crate (C5); no provenance-sdk-macros crate; no changes to
provenance-macros; no TS SDK source changes in this design (the kernel makes TS's
local computations redundant; deleting them is a later, separately-authorized
cleanup); no wire-format or protocol-version change; no wire-acceptance change (DR2's
invariant); no rejection-precedence change (DR2's pinned call placement); no
state-format changes; no crates.io publication step; no promotion, Structure, Plan, or
Implementation.

## Ownership map (post-change)

| Component | Owns after this design | Change |
|---|---|---|
| provenance-core (engine) | Domain/model/protocol types; THE AUTHORING KERNEL (builders, the full build()-time structural check set of DR1 point 2, address construction, canonical assembly for kernel-authored documents, materialization); the relocated TypedSpec*Input family; CheckStatementRequest; protocol constants | New authoring module family; input-family relocation; DR6 derives. Zero new dependencies (C3). |
| provenance-store (engine) | Identity, reconciliation, ownership, planning, locking, mutation (unchanged owners, C4), including id resolution and its resolution-dependent ensures; envelope checks (validate_typed_spec) unchanged; re-exports the input family; owns the verification-input trio; the in-process operation functions (queries, verification, enrichment incl. TypedSpecPlan, normalization, discovery with resolve_repository semantics, info); CALLS INTO the kernel at the existing pipeline points for the relocated engine-subset checks — identity.rs:35-51 structure, the declaration_ids structural ensures (:157-158), validate_references (:245-270), relationship normalization, and address inference/validation (store → kernel; no third implementation); engine identity via its version | DR4(b) absorption; new store→scanner edge; pinned kernel calls at ingestion |
| provenance-scanner (engine) | Source-site scanning, unchanged | Now also a store dependency |
| provenance-ste100 | Statement analysis; public check_descriptive is the statement operation | Unchanged |
| provenance-cli | argv/stdio/format adapters over kernel ingestion + store operations; human rendering; equivalence-suite and CLI corpus runner home | Thinned; V9 pins rendering |
| provenance-sdk (thin facade, name G4) | Re-exports; verify orchestration; macro projection; settings; parity ledger; home of V4's compile-fail suite | New, semantics-free (C5/C7) |
| provenance-macros | Inert markers | Untouched |
| TS SDK (packages/provenance) | Parity reference; additive corpus tests only; src untouched; its local assembly becomes a redundant mirror of the kernel | Test-only additions |
| workflowd vs3.9 | QRSPI promotion via supported CLI + emitted schemas | Untouched external consumer |

## Producer-identified impacts and risks (for independent review to weigh)

provenance-core's public API grows twice over (kernel + input family) — the largest
G1-audit consequence; the input-family relocation touches every in-repo consumer even
with re-exports (churn contained by re-export; blast radius verified minimal: 11
files, none outside crates/); the kernel-vs-store relationship is single-directional
by construction (the store calls into the kernel at the existing pipeline points; no
third implementation), with V2 as the divergence guard; the DR4(b) read-side
absorption carries store-growth and store→scanner coupling; TS convergence (deleting
its mirrors) is deferred — until then the mirrors persist with the kernel
authoritative; the macro_rules/const-assert projection carries its maintainability
trade; publication governance (G1), priority (G3), sync-only verification (G7) remain
open.

## Verification obligations

- V1: all existing cli_sdk* / store / core suites pass UNCHANGED — relocations and
  kernel ingestion are behavior-preserving, including the wire-acceptance-identity
  invariant and the preserved rejection precedence (DR2).
- V2: kernel-equivalence suite — same raw declarations via kernel-direct,
  wire-minimal, and wire-with-extras yield identical canonical TypedSpecInput and
  identical store outcomes; CLI output stays byte-identical
  (cli_sdk_query_scenario.rs:84-117 class); the statement check is callable with no
  repository present. Fixtures and legs: (i) a deliberately non-canonically-ordered
  document (mixed-case/non-ASCII keys) asserting response-order preservation; (ii) the
  error-case leg across ALL SIXTEEN operations — structural rejections including an
  unknown-field typed-spec document case pinning deny_unknown_fields (inputs.rs:139),
  DUPLICATE SOURCE/REQUIREMENT KEY rejection, UNDECLARED-REFERENCE rejection, STE
  rejection, ownership refusal, and schema-version refusal; (iii) documents using the
  legacy singular `requirement` field and duplicate-requirement documents, pinning
  normalize_rule_relationships semantics (merge, empty/repeat rejection, BTreeSet
  ordering) through the store → kernel call.
- V3: cross-SDK identity corpus (retire/reactivate, local↔shared migration, repeated
  keys, explicit-id merge, mixed-case keys); comparison keyed by address/stable id or
  end state.
- V4: compile-fail suite mapping the 13 TS type fixtures (error identity = code +
  identifier essence); covers the provenance_spec! const-assert link and collisions;
  checked-in parity ledger (three delta classes). HOMED in the facade crate's tests.
- V5: no-write-on-refusal preserved (cli_record_schema_versions.rs:178 discipline),
  including kernel-rejection before any store call.
- V6: Rust example CI job asserts one rule, one passed run, one binding at the Rust
  test path.
- V7 (only if G6 accepts the extension): unique symbol resolves; ambiguity/miss fails
  apply without writes, naming `.implemented_at`.
- V8: every touched crate satisfies the 500-line cap, doc budgets, workspace lints.
- V9: rendered-plan golden pins (empty, changes, affected-rules cases) added BEFORE
  the DR4(b) relocation and kept green after.
- V10: rule-binding integrity — a scanner-based test homed in provenance-store's
  tests, run post-relocation with WORKSPACE-WIDE scope, asserting every moved #[rule]
  id (rule_sdk_project_discovery; the three rule_ste_sdk_statement_*) still has a
  discovered binding site.
- V11: failing-verification corpus scenario in both languages (Err + panic in Rust;
  throw in TS) records `failed` and propagates.
- V12: kernel purity and determinism — property tests that `build()` +
  `materialize()` are deterministic over input order and perform no I/O (compile-time:
  the kernel modules import no std::fs/env/process; runtime: same declarations in any
  order yield byte-identical canonical documents).

## Alternatives rejected

All prior rejections stand where applicable (subprocess transport; provenance-engine
crate — gate-rejected; cli library target; scanner-homed or SDK-homed operations;
separate macros crate; extending provenance-macros; vacuous runtime handshake; naive
derives; "identical to TS" ordering; full-stderr snapshots; silent key mangling;
unrecorded panic propagation; kernel in a new crate; core depending on store; a new
raw wire operation / protocol v5; immediate TS rewrite; materialization in the facade;
applying the kernel's full build()-time check set at wire ingestion — it would
silently tighten wire acceptance). Revision 7 addition: floating the store's kernel
calls to new pipeline positions — rejected because it could reorder rejection
precedence for multi-defect documents; the calls are pinned at the existing check
sites (DR2).

## Gate requests (assembled for the amended package's §HUMAN GATE)

- G1 publication governance: audit scope as stated in DR10.
- G2 ratified rule-wording mapping: handshake "supported protocol range" → cargo
  semver (two coexisting enforcement readings named); dev override → cargo
  path/[patch]; query protocol_version omission semantics.
- G3 priority: proceed despite "More languages are not the next task"
  (docs/typescript-sdk-poc.md:215-218).
- G4 consumer shape and name: ratify the thin-facade determination and the facade name
  `provenance-sdk` (amendable).
- G5 promotion + structure policy pins for any real (non-dogfood) run.
- G6 DR8 severable symbol→file extension: baseline-only, or baseline + store-homed
  engine-side resolution (binds V7).
- G7 verification capability delta: sync-first with async follow-up, or async in v1.
