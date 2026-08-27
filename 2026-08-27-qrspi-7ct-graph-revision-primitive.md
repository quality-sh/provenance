---
date: 2026-08-27
bead: provenance-7ct
epic: provenance-46p
stage: structure-complete-awaiting-disposal
model: glm-5.3-flash-high
---

provenance-7ct — Decide the graph revision primitive for transactional writes (epic provenance-46p)

=== QUESTION ===

**Core question.** When `commit` publishes a previously-planned graph change, what precondition token does it compare, and what exactly does that token bind the engine to? Every other property follows from this choice: what "the graph moved" means, what forces a replan, what survives a Git clone, how merges interact, and which refusals the protocol must name.

**Sub-questions.**

1. *Token content.* Whole-graph canonical digest, digest scoped to affected record families, per-family digests hierarchically combined, Git commit SHA under a clean-tree policy, or a composite?
2. *Token scope.* Repository, scope, affected families, or the transaction's read/write sets?
3. *Token computation.* Which canonicalization produces it? Is the graph-reference projection adopted verbatim, restricted, or widened — and is an inclusion set designed for a *read pin* safe for a *write precondition*?
4. *Token storage.* Recomputed from canonical JSONL, or persisted — and does its home survive `rm -rf .provenance/cache/`, which is gitignored and rebuildable by design?
5. *Token transport.* Opaque versioned string, or structured object clients may inspect?
6. *Git participation.* Does a successful commit land a Git commit per transaction, per state class, or never under repository policy — and does token validity *depend* on Git being clean?
7. *Typed refusals.* What does the engine name when the token fails, and can current error machinery carry it?
8. *Ceremony asymmetry.* How do graph-intent writes gate while working-state edits stay ceremony-free, given both live in the same shards — and today inside the same canonical projection?

**In scope.**

- The precondition token for `plan` → `commit` of planned graph-intent changes.
- Its computation, canonicalization, scoping, storage-or-recomputation, transport.
- Whether Git commits participate in the write path and under what policy.
- The typed refusal vocabulary the token forces into existence.
- Retry/idempotence semantics for resubmitting an unchanged Change Set.
- Cross-clone determinism and interaction with the record-keyed JSONL merge driver.
- Whether working-state writes share the mechanism at lower ceremony.

**Out of scope.**

- Naming the primitive (`Planned Change` vs `Change Proposal`; research doc open decision 1); who supplies Stable IDs (decision 6); approval policy and carriers (decisions 7, 8); first Change Set contents (decision 3); durable audit records (decision 4); long-term JSONL role (decision 12).
- SQLite projection refresh mechanics beyond the token's exposure to projections.
- SDK ergonomics (that is provenance-46p.1); any implementation, child issue, or code edit.
- The refinement intersecting intervening transactions' written families against the planned change's affected set to prove safety instead of refusing.¹

**Decisions this answer must enable a reviewer to make.**

- Resolve open decision 5 ("monotonic serial, content digest, or both") with evidence rather than taste.
- Resolve open decision 9 (does a local commit always create a Git commit) for at least v1.
- Decide whether the graph-reference projection's family inclusion set is adopted, split, or widened for write preconditions — knowingly accepting the fog consequence each option implies.
- Decide scoping granularity, thereby choosing the replan rate the project buys.
- Authorize (or not) new error machinery for typed refusals; fix wire names and opacity.
- Decide whether working-state operations adopt the precondition (unchecked-approval form) immediately or later.

=== RESEARCH ===

**Evidence base, and a launcher discrepancy worth recording.** My session cwd was `/home/ben/opencode-claude-bridge`, an unrelated TypeScript project — not a Provenance checkout. I located the launcher worktree at `/home/ben/.local/share/opencode/worktrees/provenance/20260827T074638Z-77cddb80`, verified all eight briefing anchors exist there, confirmed a pristine tree at HEAD `2e88c9b` ("Merge pull request #162 … programmable-change-proposals") — the very merge introducing the cited research doc. All citations are read-only observations; nothing written or mutated. A reviewer should check cwd wiring before launching further workers.

Citations relative to worktree root. Rd = `docs/research/2026-08-27-programmable-graph-change-proposals.md`.

**1. There is no graph revision counter today. None.**
Repo-wide grep over `crates/` for `graph_revision|state_revision|revision_counter|generation|epoch|etag|expected_revision|base_revision|expected_digest`: zero state-revision hits. All 24 occurrences of `revision` are Git revisions (e.g. `crates/provenance-cli/src/wiki/links/evidence.rs:110-120,261,380-385`) or test fixture filenames (`…/wiki/publish/tests/replacement_rollback.rs:41-43`). Rd states it outright: "There is no shared graph revision that proves a projection is current" (`:323-324`). Briefing confirmed.

**2. Plan and apply are one function; no token passes between them.**
`crates/provenance-store/src/state_store/typed_specs.rs:58` `enum ReconcileMode`; `apply_typed_spec` (`:112-119`) and `plan_typed_spec` (`:122-129`) both call single `reconcile_typed_spec(scope_id, input, mode)` (`:133-137`), mode consulted only at `:143,:217` gating writes. They cannot drift. Critically both take **identical arguments** — apply never receives the plan. `TypedSpecPlan` (`crates/provenance-store/src/operations/plan.rs:21-24`) holds exactly `reconciliation` + `affected_rules` — **no digest, base token, or timestamp**. Today apply re-reconciles against whatever state it finds and writes. This is precedent for Rd invariant 4 (`:157`) and the gap invariants 5–6 (`:158-159`) demand closed.

**3. Canonical digest machinery is mature; machine independence is its declared purpose.**
`crates/provenance-store/src/graph_reference/canonical.rs:1-7`: "A graph digest and a reference id are claims about bytes somebody else holds, so the bytes have to be produced the same way every time: object keys sorted, no incidental whitespace, one SHA-256 over the result. Two machines that agree on the graph must agree on the digit string, or the pin means nothing." Pipeline: `canonical_bytes` (`:13-18`) → `write_canonical_json` (keys sorted `:28`; arrays order-preserved `:38-49`) → `digest` (`:54-56`) → `sha256:<64 hex>`. `export.rs:30-32` defines `graph_digest` once; doc `:24-29` insists on the single definition. Shape validated before hashing (`export.rs:66-70`, code `:93-104`) so illegality reports as illegality; unknown fields fail closed via `serde_ignored` (`:82-92`); mismatch names expected and actual (`:106-108`).

**4. Shipped exact-state identity is already a hybrid — commit plus digest.**
`graph_reference.rs:393-404`: `reference_identity = sha256("graph-reference-v1\0repository_id\0store_path\0scope\0commit\0graph_digest")` → `grf1_…`. `repository_id` derives from sorted root commits + store path (`graph_reference/git.rs:55-69`), identical in every clone. `docs/cli.md:347-349`: identity "idempotently derived from the Git repository roots, `.provenance/state`, scope, full commit ID, and canonical graph digest." Seeds framing "digest versus commit" is a false dichotomy against shipped code: the proven primitive is *both*. `verify_and_load` (`:293-323`) runs ordered comparisons (commit `:299-302`; repository_id `:303-306`; graph_digest `:307-311`; reference_id `:312-321`), first failure wins; doc `:285-292` notes the same reference honoured in one clone, refused in another, "which is the point."

**5. A clean-tree publication policy exists — semantic and scope-scoped, not `git status`.**
`graph_reference.rs:172-188`: implicit HEAD builds the scope projection three ways (commit `:174`, index `:176`, worktree `:177`), compares canonical bytes (`:178-181`), refuses "implicit HEAD requires clean canonical state for scope '{scope}'; commit graph changes first" (`:182-187`). `docs/cli.md:342-345`: "Implicit HEAD permits unrelated source, cache, and other-scope changes but rejects selected-scope graph changes until they are committed."

**6. The shipped digest is already partial — and its family boundary cuts through working state.**
`docs/state-format.md:115-120`: projection contains "sources, domains, requirements, boundaries, topics, questions, resolutions, rules, and edges"; excludes "threads, messages, contributions, synthesis packets, proposals, assertions, dispositions, cache data, and wiki output." Confirmed `docs/cli.md:349-352`. Naive seed A does not exist: any "whole-graph" digest is already scope/family-scoped.

This surfaces **the central contradiction of the brief.** Rd classes fog as ceremony-free working state (`:259, :269-296`; non-goal `:428`), but fog is a field on `Requirement` (`writers.rs:109-129`) and requirements are *in* the projection (`state-format.md:117-119`); likewise topics/questions projected (`:118`) yet tabulated as working state (`:259`). Consequence: preconditions on the existing digest make **one fog edit invalidate every open plan in the scope.** "Working-state stays ceremony-free while graph-intent gates" cannot hold unchanged. Notably docs partially concede the split already: "A working-state record can be written by a transaction that skips plan digests and approval while retaining ownership checks and a base-state precondition on its family" (`docs/cli.md:285-287`).

**7. Content-equality preconditions already exist at per-record grain.**
`ideation_batches.rs:263-272`: "Once any assertion cites a record, that record is frozen: a replacement is accepted only when it serializes to exactly what is already stored." Enforced `:274-289`. An **identical replacement is accepted** — exactly the idempotent-resubmission shape a Change Set retry needs. Same pattern: "A review is identified by that exact restatement, so re-applying the same change never reopens a cleared review" (`state-format.md:56-63`).

**8. Whole-set digests have one deployment here; the pain is documented.**
`ideation_batches.rs:432-448`: shipped-v1 fingerprint freezes a terminal set by SHA-256; "a row appended beside genuine history changes the fingerprint and takes the whole shard down with it." Tolerated for a frozen audit; direct evidence of seed A's cost interactively.

**9. The merge driver can mint canonical state no local transaction produced.**
`.gitattributes:1-3`: `merge=provenance-jsonl`, configured per clone via `git config merge.provenance-jsonl.driver` per `docs/cli.md:271-273`; driver op is `provenance merge-jsonl` (`:274`), also usable standalone (`:278-280`) — merge and transactional writers share one code path. Implementation `crates/provenance-store/src/merge.rs`: `MergeConflictKind::{AddAdd,DivergentEdit,DeleteModify}` (`:12-22`), conflict record carries id + base/ours/theirs (`:24-33`), outcome `Clean|Conflicted` (`:35-45`). Divergent edits conflict (`:18-21`); **disjoint additions merge cleanly** — branch A adds X, branch B adds Y, merge commits state neither branch transacted, no engine run, no lock. Merging faces write-time checks (edge endpoint recheck; STE gate) but "Other per-scope families merge without typed validation today" (`docs/cli.md:300-311`). Consequence: **any clone-local monotonic counter is refuted** — concurrent branches each mint N+1; clean merges leave no defensible continuation. Digests recompute correctly over merged state with zero special-casing.

**10. Git participation today is read-only; humans own commits.**
Non-test grep for git `commit`/`add` invocation finds nothing in engine paths (`wiki/links/remote.rs:295` is `remote add`; `handlers/cargo_init.rs:210` is `cargo add`). Store-side git callers (`graph_reference/git.rs`, `stale/git.rs`, `operations.rs`) issue reads only (`rev-parse`,`rev-list`,`ls-tree`,`ls-files`,`show`). `docs/cli.md:319-325` shows humans running `git add && git commit` before issuing references. Shallow clones refused for references (`git.rs:27-33`). Engine-written commits would be new capability tied to open decision 9 (`Rd:447-448,359-360`).

**11. Locking, staged publication, recovery can carry the design.**
Mandatory order publication → scope-lifecycle → shard; multi-shard writers hold publication lock throughout; locks "derived cache artifacts, not state, must not be committed" (`state-format.md:99-105`). Paths derive under `cache/locks/`: `layout.rs:53-62` (shard), `:63-67` (lifecycle). `publication.rs:46-65`: advisory lock, thread-reentrant via `HELD_LOCKS` (`:25-27,:57-59`), recovery before operation (`:63`). `mutate_jsonl_records` layers shard lock (`:458-470`); primitives `with_advisory_lock` (`jsonl.rs:36`), `mutate_jsonl_locked` (`jsonl.rs:52`). Marker phases Prepared/BackupCreated/Published (`:132-138`), atomic marker write (`:148-166`), recovery (`:177-208`). Honest bound: no atomic directory exchange; cooperating access never sees missing live state (`state-format.md:110-113`). Reentrancy means commit can hold the lock and still call existing writers — mechanically available for invariant 4.

**12. Typed refusals barely exist; genuinely new machinery.**
`provenance-store/src`: 122 `anyhow::ensure!/bail!` sites, zero `thiserror` enums under `state_store/`. Repo-wide store+core: three enums total — `GraphReferenceError` (`graph_reference.rs:45-57`: Missing/Mismatched{field,expected,actual}/Incomplete), `scope.rs:4`, `edge_validation.rs:4`. SDK envelope success-only (`QueryResponse<Result>` at `protocol/response.rs:18`; eight result structs `:36-109` matching Rd's eight queries `:147`); TS errors are host exceptions (`packages/provenance/src/bound-declarations.ts:333-401` et al.), i.e., failures cross as process-failure text, not protocol data. The CLI sdk surface already exposes `Plan` and `Apply` commands (`crates/provenance-cli/src/cli/sdk.rs:30-40`) — a commit operation lands beside them. Against Rd's fifteen refusal kinds (`:404-420`), the gap is structural — identical across candidates, hence costed once.

**13. Operation-specific invariant checks confirmed.**
Fog blind put: `writers.rs:111-129` (check `:117-119`, find `:122-125`, assign `:126`) — no freshness notion. Claim names holder: `shaping_writers.rs:193`. Duplicate immutables refuse: `ideation_batches.rs:353-355,:396-401,:419-425`. Adoption exact pre-match: `typed_specs/adoption.rs` `exact` = definition AND relationship match (`:106-107,:166-167,:225-227`), `rejects()` (`:311-315`) admits unowned only when adopted-and-exact (`state-format.md:9-18`; ADR `docs/adr/0008-declaration-adoption-is-explicit.md`). Identity ladder four rungs (`identity.rs:137-141`): known address → explicit id → well-formed key → *digest-suffixed slug*; ambiguity refuses (`:99`). Cross-family reads are real: reconcilers consult edges and relationship matches during source/requirement/rule reconciliation (`adoption.rs:106-107,:225-227`) and restatements raise Rule reviews (`typed_specs.rs:238-239`) touching `requirements/review.jsonl` and binding shards (`state-format.md:55-57,:35-43`). Affected-analysis embryo: `plan.rs:28-31`; ADR 0007. Dogfooding: this repository carries its own `.provenance/state/`.

**14. Seeds tested and disposed.**
- **Seed E (no engine token) — refuted for planned-change commit.** Existing checks are per-record/per-operation; none asserts "the approved semantic plan still describes execution": a restatement passes all current checks regardless of intervening Rule-side changes, though consequences differ via reviews (ADR 0007). Approval binds to semantic effects (Rd `:156`); nothing recomputes them without a token. Adequate only where no approval exists (fog, claims) — the asymmetry to preserve, not eliminate.
- **Seed D alone (commit-SHA precondition) — refuted as sole token; retained as component.** Engine never commits (finding 10), so per-transaction commits invert human/CI ownership and preempt decision 9. A SHA ignores graph content: README-only commits spuriously invalidate; merge commits legitimately rewrite state. Neither necessary nor sufficient alone. It is half the shipped `reference_identity` (`:393-404`).
- **Seed A as literally "whole-graph" — descoped by fact** (finding 6); honest form survives as Candidate A.
- **Monotonic serial — refuted** (finding 9); counters in cache are disposable (`cache.md:3,5`), counters in shards self-conflict on every concurrent branch.

---

**Evidence-split checklist**

*Repository facts (cited above):*
1. No state revision counter anywhere in `crates/`.
2. One reconciler serves plan and apply; no token exchanged; plans carry no digest/base/timestamp.
3. Canonical bytes = sorted keys, no whitespace, SHA-256; cross-machine agreement is the module's purpose; shape-before-digest; unknown fields fail closed.
4. `reference_identity` = versioned hash over repository-id ∥ store path ∥ scope ∥ commit ∥ graph digest; repository id from root commits.
5. Implicit HEAD enforces commit == index == worktree over the scope projection's canonical bytes; unrelated changes tolerated.
6. Projection includes requirements/topics/questions/edges etc.; excludes threads/messages/contributions/synthesis/proposals/assertions/dispositions/cache/wiki.
7. Fog is a `Requirement` field; set/clear is a serialized blind put; claims refuse naming holder; duplicate immutable ids refuse; adoption needs exact pre-match; identity ladder ends in digest-derived slugs.
8. Merge driver per-clone configured (`merge=provenance-jsonl`), implemented as `provenance merge-jsonl`, shareable standalone; divergent edits conflict; disjoint unions land cleanly; some families skip typed validation; docs sketch family-scoped low-ceremony transactions skipping plan digests.
9. Engine shells git read-only; humans own commits; shallow history refused for references.
10. Lock order publication→lifecycle→shard mandatory; publication lock reentrant; markers recoverable; locks live under gitignored cache.
11. Refusals overwhelmingly stringly-typed (anyhow ×122 vs thiserror ×0 in state_store); SDK envelope success-only; TS errors are host exceptions; plan/apply already protocol-exposed.
12. Exact-serialization preconditions exist (assertion-cited evidence; restatement reviews), identity-accepting, difference-refusing.
13. Reconcilers read across families (edges, bindings, reviews) during reconciliation and restatement analysis.
14. Decisive-prototype contract: `Change Set + base revision + expected digest + approvals` (Rd `:486-490`); prototype must prove equal-input/equal-base ⇒ equal digest, stale-base typed refusal, atomic multi-record publish (Rd `:492-503`).

*My inference (reason, observation, extrapolation — not citable):*
15. Monotonic counters cannot survive distributed correctness — logical entailment of fact 8.
16. Fog-inside-projection ⇒ any stateful-digest precondition poisons ceremony-free flows — analysis over facts 6–7; resolution is a values question.
17. Candidate scoring weighs hypothetical agent-interleaving patterns; no usage telemetry exists in-repo.
18. Read/write-set closure per operation is unknown — auditable evidence shows cross-family reads (fact 13) but a full closure proof per operation has not been attempted; flagged as obligation if B/C chosen.
19. Session retrieval-context risk: cwd/worktree mismatch resolved via permission allowlists plus anchor presence and HEAD containing the briefing's own anchor document; residual risk judged low.

=== STRUCTURE ===

Four surviving candidates follow, each addressing: interleave annoyance (what forces replan, when), retry semantics for an unchanged Change Set, determinism across clones, JSONL merge-driver interaction, and why working-state edits stay ceremony-free while graph-intent writes gate. Typed-refusal machinery (finding 12) is common and costed once. No winner picked.

---

**Candidate A — Whole-projected-scope digest (adopt the graph-reference digest as precondition).**

*Position.* Precondition = existing `sha256:` digest over the target scope's v1 projection. `plan(ChangeSet, scope)` computes D(state); `commit(ChangeSet, scope, D, approvals)` recomputes D under the publication lock; refuse unless equal.

*Mechanism sketch.* Reuse `canonical_bytes`/`digest` verbatim — single-definition doctrine preserved by construction. Invariants: equal input + equal base ⇒ equal digest free (Rd invariant 2); recompute-under-lock uses reentrancy (finding 11); stored nowhere, recomputed always; transport opaque, self-versioned string. Refusal mirrors `GraphReferenceError::Mismatched{field:"base_graph_digest", expected, actual}` (`graph_reference.rs:50-54,342-352`).

*Preserves.* Lock order and recovery untouched; one-digest doctrine; blind-put writers untouched; canonical vocabulary stays read-pin == write-precondition, one concept fewer.

*Interleave annoyance.* Maximal. Any projected-family change in scope — fog edit, claim, opened question (all projected, fact 6) — invalidates open plans regardless of relevance. Ordinary shaping poisons planning whenever they share a scope.

*Retry.* Unchanged Change Set + unchanged base ⇒ byte-identical digest; idempotent acceptance, mirroring the exact-serialization precedent (`ideation_batches.rs:274-289`). Moved base ⇒ replan-and-reapprove, then same Change Set replays.

*Clone determinism.* Strongest footing possible — the module exists so two agreeing machines print the same digits (`canonical.rs:1-7`).

*Merge driver.* Clean union changes D ⇒ honest refuse-and-replan; conflicts leave D unusable until resolved. Zero special cases.

*Ceremony asymmetry.* Structurally broken: fog keeps no approval yet gates others' planned commits. Ceremony-free writes purchase cross-actor replans. Only defect: docs' family-scoped sketch (`cli.md:285-287`) cannot be honored here.

*What makes it wrong.* If shaping/planning routinely interleave, refusal pressure rewards hold-the-lock mega-changes, eroding review quality — maximum invalidation strength bought where minimum suffices.

---

**Candidate B — Digest scoped to affected record families.**

*Position.* Precondition = per-family digests over only the families the normalized plan touches, combined version-tagged: `dgb1\0scope\0fam=digest,…`.

*Mechanism sketch.* Family list derives mechanically from normalized operations plus an engine-declared closure rule (new edge extends to endpoint families; requirement restatement extends to rules/review/bindings per ADR 0007 and finding 13). Each component digests by restricting the existing canonicalizer to the family subset — a restriction, not a second serialization dialect. Commit recomputes exactly these components under the lock. Refusal: `StaleFamilies{families:[…], per_family:{expected,actual}}`, naming where state moved.

*Preserves.* Canonical-byte discipline (restriction of one function); ownership/adoption checks preceding writes; retire-in-place captured intra-family; multi-record atomicity via staged publication.

*Interleave annoyance.* Sharply reduced: unfamily'd churn stops forcing replans. Fog still bites when the requirement family is touched — which restatement plans inherently are. Disjoint records in the *same* family still collide (family granularity, not record granularity).

*Retry.* Unchanged Change Set ⇒ same family set, same components ⇒ idempotent admission. Partial drift names the guilty family, telling clients whether replanning could plausibly help.

*Clone determinism.* Holds iff family enumeration is closed and versioned — frame embeds `dgb1`, so future inclusion changes mint incomparable tokens rather than silently accepting stale ones (versioning discipline of `graph-reference-v1`).

*Merge driver.* Merge touching a watched family ⇒ refuse/replan; elsewhere ⇒ proceed. Correctness rests entirely on closure being exhaustive.

*What makes it wrong.* Soundness is a proof obligation: **any hidden validator dependency on an excluded family turns B unsound silently**, admitting commits whose executed effects differ from approved ones (violating Rd invariants 5–6 quietly instead of loudly). Fact 13 confirms cross-family reads exist; fact 18 says the closure audit hasn't been run. If proving per-operation closure is impractical, B is disqualified despite best ergonomics.

---

**Candidate C — Per-family digest lattice, hierarchically combined; doubles as freshness metadata.**

*Position.* Continuously-defined digests per family, foldable into scope/root aggregates. Planned-change precondition = vector covering the transaction's read/write sets; SQLite projections stamp the same values, giving Rd invariants 11–12 (`:163-165`) a native carrier.

*Mechanism sketch.* Values strictly derived (recomputed bottom-up), never authoritative state (counters in shards violate the no-volatile-fields law and merge poorly; cached copies advisory). Projections declare which vector they rendered; readers compare before serving. Per-component comparison yields diagnosis plus refusal.

*Preserves.* Cache-never-source-of-truth (`cache.md:5`) — overlay is rebuildable with the db; lock discipline unchanged; derivation under existing snapshots.

*Interleave annoyance.* Best locality at reasonable complexity: components isolate movement; other scopes invisible; shaping vs planning separated if their families differ. Intra-family collision caveat persists as in B, plus B's full closure burden.

*Retry.* Vector equality ⇒ admission; partial matches localize replan necessity. Idempotent for unchanged inputs.

*Clone determinism.* Same content-derived footing as A/B provided nothing wall-clock or ordinal enters aggregation — which the lattice forbids by construction, at the cost of also blocking "revision N → N+1" storytelling without a separate counter (Rd flow diagram `:170-194` would need reinterpretation).

*Merge driver.* Post-merge recomputation trivially local (leaves rehash, ancestors fold). Conflicts fail before publication, so lattices never observe conflicted intermediates.

*What makes it wrong.* Complexity creep: maintaining component equivalence across canonicalizer evolution; inheriting B's soundness problem multiplied by projection consumers trusting components. Over-engineering risk for v1 is the candid criticism — several benefits accrue to projections/queries outside this bead's commitment.

---

**Candidate D — Git-clean-tree hybrid (exact-state token per repository policy).**

*Position.* Adopt `reference_identity` wholesale: `(repository_id, store_path, scope, commit, graph_digest)`; validity demands commit-currency plus digest-equality over clean scopes; Git landing remains policy, not implication.

*Mechanism sketch.* Commit re-runs `issue`'s procedure under the lock for involved scopes: resolve HEAD (refusing shallow, `git.rs:27-33`), compare canonical bytes across commit/index/worktree, recompute repository id and digest, compare framed token component-wise like `verify_and_load`'s ordered mismatches. Repository-policy switches decide whether successful publication requests the companion Git commit from humans/CI (engine stays write-free, keeping decision 9 open).

*Preserves.* Total alignment with existing exact-state vocabulary — references and exports remain artifacts of committed graphs; strongest audit story: a committed transaction pins *where* (commit) as well as *what* (digest).

*Interleave annoyance.* Pathological in one precise way: the commit component fires regardless of whether the *graph* moved. Any commit — README, Cargo.lock — voids open tokens even in frozen scopes. The cure exists narrowly (`issue` checks only selected-scope canonical bytes, fact 5); replicating narrowness demotes commit-mismatch to advisory metadata, collapsing D into A plus provenance logging.

*Retry.* All parts revalidate ⇒ accept; any part moved ⇒ named by ordered `Mismatched{field:…}`. Idempotent, diagnosable.

*Clone determinism.* Established mechanism (`reference_identity`, root-commit hashing); inherits shallow-clone exclusion — an operational cost in ephemeral CI checkouts absent from A/B/C.

*Merge driver.* Landed merges move commit and digest together beyond either side's anticipation — behavior equals A's refuse/replan, compounded by unrelated commits landing mid-flight.

*What makes it wrong.* Strict form couples Provenance concurrency to unrelated repository cadence — false refusals track overall commit volume. Loose/advisory form is indistinguishable from A decorated with provenance. Residual value is an *audit annotation*, not a gate.

---

**Cross-candidate constants (costed once).**

- *Typed refusals.* All four need elevating stale-class failures from `anyhow` strings to an enum carrying `field/expected/actual`, threaded through an SDK envelope that currently owns no error variant (`response.rs:18`), with working-state writers migrating case by case. Whether to extend `GraphReferenceError` or mint a transaction-domain enum is a naming decision for the reviewer.
- *Determinism doctrine.* Every viable candidate inherits `canonical.rs` discipline; none invents a second hasher.
- *Lock integration.* All gate recomputation inside `with_repository_publication`, relying on reentrancy; none alters lock order.
- *Merge correctness.* All treat post-merge state as moved for watched components — forced by finding 9 regardless of candidate.
-¹ *Footnote (explicitly out of scope per brief):* the later refinement intersecting intervening transactions' written families against the planned change's affected set could certify many currently-refused commits safe, shrinking the interleave annoyance for B/C substantially — recorded so its absence is a known trade-off, not an oversight.

**Decisions reserved explicitly to the human reviewer.**

1. Choose among A (simplest, roughest edges), B (scoped; conditional on a closure audit never performed), C (most machinery, best locality, pays off projections too), D (provenance-rich; churn-hostile if strict, decays to A if loose). No ranking offered.
2. Resolve the projection-boundary contradiction: are fog (and open Topics/Questions) inside the *write-precondition* universe? Visible options: split write-time projection from reference projection; relocate fog out of `Requirement`; accept cross-actor replans as the price of one canonicalization. Note docs already gesture at family-scoped low-ceremony writes (`cli.md:285-287`).
3. If B or C favored: authorize the closure-proof programme — enumerate each operation's true read/write families (evidence exists they span edges/bindings/reviews), registry-checked, conservative wide default for unaudited operations.
4. Fix Git participation for v1: keep the engine commit-free (today's reality) with landing owned by humans/CI, or consciously open decision 9 for specific state classes, accepting shallow-CI constraints.
5. Set the refusal-machinery budget: new typed enum + envelope error channel versus extending `GraphReferenceError`; fix wire names (`base_revision` vs `expected_state_token`) and opacity.
6. Decide whether the first kernel applies the precondition across state classes (working-state ops checked but unapproved, per docs' sketch) or ships graph-intent-only initially.
7. Confirm the accepted v1 trade-off: unrelated working-state edits may force replan; remedy deferred to the footnote refinement.
