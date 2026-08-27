---
date: 2026-08-27
bead: provenance-7ct
epic: provenance-46p
stage: structure-complete-awaiting-disposal
model: glm-5.3-flash-high
---

provenance-7ct — Decide the graph revision primitive for transactional writes (epic provenance-46p)

=== QUESTION ===

**Core question.** The engine publishes a planned graph change at `commit`. Before publication, `commit` compares one precondition token against current graph state. Which token does `commit` compare? And what does that comparison bind the engine to? One choice decides everything else:

- what "the graph moved" means;
- what forces a replan;
- what survives a Git clone;
- how merges affect the choice;
- which refusals the protocol must name.

**Sub-questions.**

1. *Token content.* Is the precondition a whole-graph canonical digest? A digest over only the touched record families? Per-family digests combined in a hierarchy? A Git commit SHA under a clean-tree policy? Or several of these together?
2. *Token scope.* Does it cover the repository, the scope, the affected families, or the read and write sets of the transaction?
3. *Token computation.* Which canonicalization computes it? Does the design adopt the graph-reference projection as-is, restrict it, or widen it? A family set built for a *read pin* may be unsafe as a *write precondition*. Is this set safe here?
4. *Token storage.* Does the engine compute the token again from canonical JSONL, or store it somewhere? If stored: does its location survive `rm -rf .provenance/cache/`? That directory is gitignored and rebuildable by design.
5. *Token transport.* Is the token an opaque versioned string? Or a structured object that clients can inspect?
6. *Git participation.* Does a successful commit produce a Git commit each time, sometimes by state class, or never under repository policy? Does token validity depend on a clean Git tree?
7. *Typed refusals.* What does the engine report when the check fails? Can today's error machinery carry that report?
8. *Ceremony asymmetry.* Graph-intent writes must pass approval checks. Working-state edits must not pass approval checks. Both kinds live in the same shards. Today both also live inside the same canonical projection. How can one token satisfy both demands?

**In scope.**

- The precondition token for `plan` then `commit` of planned graph-intent changes.
- Its computation, canonicalization, scoping, storage-or-recomputation, and transport.
- Whether Git commits take part in the write path, and under what policy.
- The typed refusal set that this token creates.
- Retry and idempotence rules when a client resends an unchanged Change Set.
- Determinism across Git clones. Interaction with the record-keyed JSONL merge driver.
- Whether working-state writes share the same mechanism at lower ceremony.

**Out of scope.**

- Naming the primitive: `Planned Change` versus `Change Proposal`. Research doc open decision 1.
- Who supplies new Stable IDs (decision 6). Approval policy and carriers (decisions 7, 8). First Change Set contents (decision 3). Durable audit records (decision 4). Long-term role of JSONL (decision 12).
- SQLite projection refresh mechanics, except how the token appears to projections.
- SDK ergonomics. That work belongs to provenance-46p.1. Also excluded: any implementation, child issue, or code edit.
- The later refinement compares intervening transactions' written families against the planned change's affected set. It could prove safety instead of refusing.¹

**Decisions this answer must enable a reviewer to make.**

- Resolve open decision 5 ("monotonic serial, content digest, or both"). Use evidence, not taste.
- Resolve open decision 9. Does a local commit always create a Git commit? Decide at least for v1.
- Adopt, split, or widen the projection's family inclusion set for write preconditions. Each option implies one fog consequence. Accept that consequence knowingly.
- Decide scoping granularity. That decision sets how often actors replan.
- Authorize or refuse new error machinery for typed refusals. Fix wire names and opacity.
- Decide whether working-state operations adopt the precondition now or later.

=== RESEARCH ===

**Evidence base, and one launcher note.** My session started in `/home/ben/opencode-claude-bridge`. That directory is an unrelated TypeScript project, not Provenance. I located the launcher worktree at `/home/ben/.local/share/opencode/worktrees/provenance/20260827T074638Z-77cddb80`. All eight briefing anchors exist there. The tree is clean. HEAD is `2e88c9b`, "Merge pull request #162 … programmable-change-proposals". That merge introduced the research doc cited below. Every citation below comes from read-only checks of that worktree. Nothing was written or mutated. A reviewer should verify cwd wiring before further workers start.

Citations are relative to worktree root. Rd = `docs/research/2026-08-27-programmable-graph-change-proposals.md`.

**1. No graph revision counter exists today.**
A search across `crates/` looked for `graph_revision|state_revision|revision_counter|generation|epoch|etag|expected_revision|base_revision|expected_digest`. It found no state revision. All 24 matches for `revision` name Git revisions (`crates/provenance-cli/src/wiki/links/evidence.rs:110-120,261,380-385`) or test fixture filenames (`…/wiki/publish/tests/replacement_rollback.rs:41-43`). Rd states the same negative: "There is no shared graph revision that proves a projection is current" (`:323-324`). The briefing is confirmed.

**2. Plan and apply share one function. No token passes between them.**
`crates/provenance-store/src/state_store/typed_specs.rs:58` defines `enum ReconcileMode`. `apply_typed_spec` (`:112-119`) and `plan_typed_spec` (`:122-129`) both call one shared `reconcile_typed_spec(scope_id, input, mode)` (`:133-137`). Code consults the mode only at `:143,:217` to permit or block writes. The two paths cannot drift. Both take identical arguments. Apply never receives the plan. `TypedSpecPlan` (`crates/provenance-store/src/operations/plan.rs:21-24`) holds exactly `reconciliation` plus `affected_rules`. It holds no digest, base token, or timestamp. Today apply reconciles against whatever state it finds, then writes. This is precedent for Rd invariant 4 (`:157`). Invariants 5–6 (`:158-159`) demand closure of the gap.

**3. The canonical digest machinery is mature. Machine independence is its declared purpose.**
`crates/provenance-store/src/graph_reference/canonical.rs:1-7` states: "A graph digest and a reference id are claims about bytes somebody else holds, so the bytes have to be produced the same way every time: object keys sorted, no incidental whitespace, one SHA-256 over the result. Two machines that agree on the graph must agree on the digit string, or the pin means nothing." Pipeline: `canonical_bytes` serializes (`:13-18`). `write_canonical_json` sorts object keys (`:28`) and keeps array order (`:38-49`). `digest` writes `sha256:<64 hex>` (`:54-56`). `export.rs:30-32` defines `graph_digest` once. Its doc (`:24-29`) demands that single definition. Code validates shape before hashing (`export.rs:66-70`, code `:93-104`). Illegal documents therefore report illegality, not hash failure. Unknown fields fail closed through `serde_ignored` (`:82-92`). A mismatch names expected and actual (`:106-108`).

**4. Shipped exact-state identity already combines commit and digest.**
`graph_reference.rs:393-404`: `reference_identity = sha256("graph-reference-v1\0repository_id\0store_path\0scope\0commit\0graph_digest")` → `grf1_…`. `repository_id` derives from sorted root commits plus store path (`graph_reference/git.rs:55-69`). Root commits are identical in every clone. `docs/cli.md:347-349`: identity "idempotently derived from the Git repository roots, `.provenance/state`, scope, full commit ID, and canonical graph digest." The seed framing "digest versus commit" contradicts shipped code. The proven primitive uses both. `verify_and_load` (`:293-323`) runs ordered comparisons: commit (`:299-302`), repository_id (`:303-306`), graph_digest (`:307-311`), reference_id (`:312-321`). First failure wins. Its doc (`:285-292`) notes one effect: clones can honour the same reference differently, "which is the point."

**5. A clean-tree publication policy exists. It is semantic and scope-scoped, not `git status`.**
`graph_reference.rs:172-188`: implicit HEAD builds the scope projection three ways. It reads the commit (`:174`), the index (`:176`), and the worktree (`:177`). It compares canonical bytes (`:178-181`). On mismatch it refuses "implicit HEAD requires clean canonical state for scope '{scope}'; commit graph changes first" (`:182-187`). `docs/cli.md:342-345`: "Implicit HEAD permits unrelated source, cache, and other-scope changes but rejects selected-scope graph changes until they are committed."

**6. The shipped digest is already partial. Its family boundary cuts through working state.**
`docs/state-format.md:115-120`: the v1 projection contains "sources, domains, requirements, boundaries, topics, questions, resolutions, rules, and edges". It excludes "threads, messages, contributions, synthesis packets, proposals, assertions, dispositions, cache data, and wiki output." `docs/cli.md:349-352` confirms both lists. So naive seed A does not exist. Every "whole-graph" digest already scopes by scope and family.

This exposes the central contradiction of the brief. Rd classes fog as working state written without approval ceremony (`:259, :269-296`; non-goal `:428`). But fog is a field on `Requirement` (`writers.rs:109-129`). Requirements sit inside the projection (`state-format.md:117-119`). Topics and questions also project (`:118`), yet the table calls them working state (`:259`). Consequence: if commit preconditions on the existing digest, one fog edit invalidates every open plan in the scope. Working-state edits cannot stay ceremony-free while graph-intent writes gate, unless something splits. Docs already sketch that split: "A working-state record can be written by a transaction that skips plan digests and approval while retaining ownership checks and a base-state precondition on its family" (`docs/cli.md:285-287`).

**7. Content-equality preconditions exist at per-record grain today.**
`ideation_batches.rs:263-272`: "Once any assertion cites a record, that record is frozen: a replacement is accepted only when it serializes to exactly what is already stored." Enforced at `:274-289`. Note the retry shape. An identical replacement passes. Only a different replacement refuses. This matches what an unchanged Change Set retry needs. Same pattern elsewhere: "A review is identified by that exact restatement, so re-applying the same change never reopens a cleared review" (`state-format.md:56-63`).

**8. Whole-set digests have one deployment here. Its cost is documented.**
`ideation_batches.rs:432-448`: the shipped-v1 fingerprint freezes a terminal set by SHA-256. "Membership is a property of the whole set, so a row appended beside genuine history changes the fingerprint and takes the whole shard down with it." A frozen audit tolerates this cost. Interactive planning likely would not. This shows seed A's price directly.

**9. The merge driver can create canonical state that no local transaction produced.**
`.gitattributes:1-3`: `.provenance/state/**/*.jsonl merge=provenance-jsonl`. Each clone configures the driver once (`docs/cli.md:271-273`). The operation is `provenance merge-jsonl` (`:274`). It also runs standalone (`:278-280`). Merge and transactional writers share one code path. Implementation at `crates/provenance-store/src/merge.rs`: `MergeConflictKind::{AddAdd,DivergentEdit,DeleteModify}` (`:12-22`). Each conflict carries id plus base, ours, theirs (`:24-33`). Outcome is `Clean|Conflicted` (`:35-45`). Divergent edits conflict; they never union silently (`:18-21`). But disjoint additions merge cleanly. Branch A adds X. Branch B adds Y. The merge commits state that neither branch transacted. No engine ran. No lock applied. Merge output faces write-time checks (edge endpoint recheck; STE gate), but "Other per-scope families merge without typed validation today" (`docs/cli.md:300-311`). Consequence: any clone-local monotonic counter fails. Concurrent branches each mint N+1. Clean merges leave no defensible successor value. Digests recompute correctly over merged state with zero special cases.

**10. Git participation today is read-only. Humans own commits.**
Searches found no git `commit` or `add` invocation outside tests (`wiki/links/remote.rs:295` is `remote add`; `handlers/cargo_init.rs:210` is `cargo add`). Store-side git callers are `graph_reference/git.rs`, `stale/git.rs`, and `operations.rs`. They issue reads only: `rev-parse`, `rev-list`, `ls-tree`, `ls-files`, `show`. `docs/cli.md:319-325` shows humans running `git add && git commit` before issuing references. References refuse shallow clones (`git.rs:27-33`). Engine-written commits would be new capability, tied to open decision 9 (`Rd:447-448,359-360`).

**11. Locking, staged publication, and recovery can carry the design.**
Lock order is mandatory: repository publication lock, then scope-lifecycle lock, then shard lock (`state-format.md:99-105`). Multi-shard writers hold the publication lock throughout. Locks are "derived cache artifacts, not state, must not be committed". Paths live under `cache/locks/`: shard at `layout.rs:53-62`, lifecycle at `:63-67`. `publication.rs:46-65` takes an advisory lock. It reenters per thread through `HELD_LOCKS` (`:25-27,:57-59`). Recovery runs before the operation (`:63`). `mutate_jsonl_records` adds the shard lock (`:458-470`). Primitives: `with_advisory_lock` (`jsonl.rs:36`), `mutate_jsonl_locked` (`jsonl.rs:52`). Markers use phases Prepared, BackupCreated, Published (`:132-138`). Marker writes are atomic (`:148-166`). Recovery handles them (`:177-208`). One honest limit: no atomic directory exchange exists. Cooperating access never sees missing live state (`state-format.md:110-113`). Reentrancy matters: commit can hold the publication lock and still call existing writers. Mechanism for invariant 4 is ready.

**12. Typed refusals barely exist. New machinery is genuinely required.**
`provenance-store/src` holds 122 `anyhow::ensure!/bail!` sites. Zero `thiserror` enums sit under `state_store/`. Store and core hold three typed enums total: `GraphReferenceError` (`graph_reference.rs:45-57`: Missing, Mismatched{field,expected,actual}, Incomplete), `scope.rs:4`, `edge_validation.rs:4`. The SDK envelope carries success only (`QueryResponse<Result>` at `protocol/response.rs:18`; eight result structs `:36-109` matching Rd's eight queries `:147`). TS errors stay host-side exceptions (`packages/provenance/src/bound-declarations.ts:333-401` and kin). Failures cross as process-failure text, not protocol data. The CLI sdk surface already exposes `Plan` and `Apply` (`crates/provenance-cli/src/cli/sdk.rs:30-40`). A commit command lands beside them. Rd demands fifteen refusal kinds (`:404-420`). Today's gap is structural. The gap is identical across candidates, so this section costs it once.

**13. Operation-specific invariant checks confirmed.**
Fog put: `writers.rs:111-129`. Check emptiness (`:117-119`), find record (`:122-125`), assign (`:126`). No freshness notion applies. Claims: refusal names the holder (`shaping_writers.rs:193`). Duplicate immutable ids refuse (`ideation_batches.rs:353-355,:396-401,:419-425`). Adoption needs exact pre-match: `typed_specs/adoption.rs` defines `exact` as definition match AND relationship match (`:106-107,:166-167,:225-227`). `rejects()` (`:311-315`) admits unowned records only when adopted-and-exact (`state-format.md:9-18`; ADR `docs/adr/0008-declaration-adoption-is-explicit.md`). Identity ladder has four rungs (`identity.rs:137-141`): known address, explicit id, well-formed key, digest-suffixed slug. Ambiguity refuses (`:99`). Cross-family reads are real: reconcilers consult edges and relationships while reconciling sources, requirements, rules (`adoption.rs:106-107,:225-227`). Restatements raise Rule reviews (`typed_specs.rs:238-239`) touching `requirements/review.jsonl` and binding shards (`state-format.md:55-57,:35-43`). Affected-set analysis starts small: `plan.rs:28-31`, ADR 0007. Dogfooding: this repository carries its own `.provenance/state/`.

**14. Seeds tested and disposed.**
- **Seed E (no engine token) fails for planned-change commit.** Current checks run per record and per operation. None asserts this: "the approved semantic plan still describes execution". Example: a restatement passes every current check regardless of intervening Rule-side changes, yet reviews change consequences (ADR 0007). Approval binds to semantic effects (Rd `:156`). Nothing recomputes effects without a token. Seed E works only where approval never exists: fog, claims. That asymmetry must survive design, so seed E stays insufficient here.
- **Seed D alone (commit-SHA precondition) fails as sole token; survives as component.** The engine never commits (finding 10). Per-transaction commits would reverse human and CI ownership and preempt decision 9. A SHA ignores graph content: README-only commits invalidate without cause; merge commits rewrite state legitimately. Neither necessary nor sufficient alone. Yet a SHA is half the shipped `reference_identity` (`:393-404`).
- **Seed A as literal "whole-graph" loses scope by fact** (finding 6). Its honest form survives as Candidate A.
- **Monotonic serial fails** (finding 9). Counters in cache are disposable (`cache.md:3,5`). Counters in shards conflict on every concurrent branch.

---

**Evidence-split checklist**

*Repository facts (cited above):*
1. No state revision counter exists anywhere in `crates/`.
2. One reconciler serves plan and apply. No token moves between them. Plans carry no digest, base, or timestamp.
3. Canonical bytes sort keys, drop whitespace, hash once with SHA-256. Cross-machine agreement is the module's purpose. Shape precedes digest. Unknown fields fail closed.
4. `reference_identity` hashes a versioned frame: repository-id ∥ store path ∥ scope ∥ commit ∥ graph digest. Repository id comes from root commits.
5. Implicit HEAD requires commit == index == worktree over the scope projection's canonical bytes. Unrelated changes stay tolerated.
6. The projection includes requirements, topics, questions, edges, and more. It excludes threads, messages, contributions, synthesis packets, proposals, assertions, dispositions, cache, wiki.
7. Fog is a `Requirement` field. Set and clear are serialized blind puts. Claims refuse and name the holder. Duplicate immutable ids refuse. Adoption needs exact pre-match. Identity ends in digest-derived slugs.
8. Clones configure `merge=provenance-jsonl` per clone. It implements `provenance merge-jsonl` and runs standalone. Divergent edits conflict. Disjoint unions land cleanly. Some families skip typed validation. Docs sketch low-ceremony transactions that skip plan digests and precondition per family.
9. The engine calls git read-only. Humans own commits. References refuse shallow history.
10. Lock order publication → lifecycle → shard is mandatory. The publication lock reenters. Markers recover. Locks live under gitignored cache.
11. Store refusals are mostly stringly-typed: anyhow ×122 versus thiserror ×0 in state_store. The SDK envelope carries success only. TS errors stay host exceptions. Plan and apply are protocol-exposed.
12. Exact-serialization preconditions exist for assertion-cited evidence and restatement reviews. Identical replacements pass. Different ones refuse.
13. Reconcilers read across families: edges, bindings, reviews, during reconciliation and restatement analysis.
14. Decisive prototype contract: `Change Set + base revision + expected digest + approvals` (Rd `:486-490`). It must prove equal-input/equal-base gives equal digest, stale-base typed refusal, atomic multi-record publish (Rd `:492-503`).

*My inference (reason, observation, extrapolation — not citable):*
15. Monotonic counters fail distributed correctness. Logical entailment of fact 8's evidence.
16. Fog inside projection means any stateful-digest precondition harms ceremony-free flows. Analysis of facts 6–7. Resolution is a values question.
17. Candidate scoring weighs hypothetical agent interleaving. No usage telemetry exists in-repo.
18. Read/write-set closure per operation is unknown. Evidence shows cross-family reads (fact 13). No full closure proof has run. Flagged as obligation if B or C is chosen.
19. Session retrieval risk: cwd/worktree mismatch resolved through permission allowlists, anchor presence, and HEAD containing the briefing's own anchor document. Residual risk judged low.

=== STRUCTURE ===

Four candidates survive. Each candidate section covers seven points: position, mechanism sketch, preserved behaviour, interleave cost (what forces a replan, and when), retry behaviour for an unchanged Change Set, determinism across clones, interaction with the JSONL merge driver, and why working-state edits skip ceremony while graph-intent writes require it. Typed-refusal machinery is common to all four and costs once. No winner is picked.

---

**Candidate A — Whole-projected-scope digest (adopt the graph-reference digest as precondition).**

*Position.* The precondition equals the existing `sha256:` digest over the target scope's v1 projection. `plan(ChangeSet, scope)` computes D from state. `commit(ChangeSet, scope, D, approvals)` recomputes D under the publication lock. Refuse unless equal.

*Mechanism sketch.* Reuse `canonical_bytes` and `digest` verbatim. The single-definition doctrine holds by construction. Equal Change Set plus equal base gives equal digest. Rd invariant 2 then holds without added work. Recompute sits inside `with_repository_publication` (finding 11), using its reentrancy. Storage: none; recomputed always. Transport: opaque self-versioned string. Refusal mirrors `GraphReferenceError::Mismatched{field:"base_graph_digest", expected, actual}` (`graph_reference.rs:50-54,342-352`).

*Preserves.* Lock order and recovery untouched. Single-digest doctrine untouched. Blind-put writers untouched. Read pins and write preconditions share one vocabulary. One concept fewer overall.

*Interleave annoyance.* Maximum here. Any change to a projected family invalidates open plans — fog edit, claim, opened question (all projected, fact 6). When shaping and planning share a scope, ordinary shaping invalidates those plans.

*Retry.* Unchanged Change Set plus unchanged base yields byte-identical digest. Admission is idempotent, matching the exact-serialization precedent (`ideation_batches.rs:274-289`). Moved base forces replan-and-reapprove, then the same Change Set replays.

*Clone determinism.* The strongest base available. The module exists so two agreeing machines print the same digits (`canonical.rs:1-7`).

*Merge driver.* A clean union changes D, forcing honest refuse-and-replan. Conflicts keep D unusable until resolution. No special cases anywhere.

*Ceremony asymmetry.* Structurally broken. Fog needs no approval, yet every fog edit invalidates other actors' planned commits. Ceremony-free writes cause replans for other actors. Also: docs' family-scoped sketch (`cli.md:285-287`) cannot be honoured here.

*What makes it wrong.* Suppose shaping and planning routinely interleave. Refusal pressure then encourages large write batches taken under one lock. Review quality degrades as a result. It applies maximum invalidation strength where minimum suffices.

---

**Candidate B — Digest scoped to affected record families.**

*Position.* The precondition contains per-family digests for only the families the normalized plan touches. Frame: `dgb1\0scope\0fam=digest,…`.

*Mechanism sketch.* Normalized operations determine the family list mechanically. An engine-declared closure rule extends it: new edges extend to endpoint families; requirement restatement extends to rules, review, bindings (ADR 0007, finding 13). Components restrict the existing canonicalizer to a family subset. Restriction, not a second dialect. Commit recomputes exactly these components under the lock. Refusal: `StaleFamilies{families:[…], per_family:{expected,actual}}`, naming where state moved.

*Preserves.* Canonical-byte discipline as restriction of one function. Ownership and adoption checks precede writes. Retire-in-place lives intra-family (`retired:true` in the same family, captured). Multi-record atomicity rides staged publication.

*Interleave annoyance.* Sharply reduced. Unfamily'd churn stops forcing replans. Fog still triggers replans when the requirement family is touched; restatement plans inherently touch it. Disjoint records inside one family still collide: granularity is family-level, not record-level.

*Retry.* Unchanged Change Set yields same family set and components. Idempotent admission results. Partial drift names the guilty family, telling clients whether replanning could help.

*Clone determinism.* Holds only if family enumeration stays closed and versioned. The frame embeds `dgb1`. Future inclusion changes mint incomparable tokens rather than accepting stale ones. Versioning discipline follows `graph-reference-v1`.

*Merge driver.* Merges touching watched families force refuse/replan. Elsewhere commit proceeds. Correctness rests entirely on closure completeness.

*What makes it wrong.* Soundness becomes a proof obligation. Any hidden validator dependency on excluded families makes B unsound silently. Commits would execute different effects than approved, breaking Rd invariants 5–6 quietly instead of loudly. Fact 13 proves cross-family reads exist. Fact 18 says closure is unproven. If proving per-operation closure is impractical, disqualify B despite best ergonomics.

---

**Candidate C — Per-family digests combined hierarchically; doubles as freshness metadata.**

*Position.* Each family carries a continuously-defined digest of its canonical slice. Scope and root aggregates fold children together. Planned-change precondition = vector covering the transaction's read and write sets. SQLite projections stamp the same values, giving Rd invariants 11–12 (`:163-165`) a native carrier.

*Mechanism sketch.* Values derive strictly, recomputed bottom-up. Nothing stores them as authoritative state. Counters in shards violate the no-volatile-fields law and merge poorly; cached copies stay advisory. Projections declare which vector they rendered. Readers compare vectors before serving. Per-component comparison yields diagnosis plus refusal.

*Preserves.* Cache stays non-authoritative (`cache.md:5`). Overlay rebuilds alongside db. Lock discipline unchanged. Derivation rides existing snapshots.

*Interleave annoyance.* Best locality at reasonable complexity. Components isolate movement. Other scopes invisible. Shaping separates from planning when their families differ. Intra-family collision caveat persists as in B, adding B's full closure burden.

*Retry.* Vector equality admits. Partial matches localize whether replanning helps. Idempotent for unchanged inputs.

*Clone determinism.* Same content-derived footing as A/B, provided nothing wall-clock or ordinal enters aggregation. Construction forbids such inputs by itself. Cost: "revision N → N+1" storytelling blocks (Rd flow diagram `:170-194` needs reinterpretation).

*Merge driver.* Post-merge recomputation localizes trivially. Leaves rehash, ancestors fold up. Conflicts fail before publication, so lattices never observe conflicted intermediates.

*What makes it wrong.* Complexity creep threatens. Component equivalence across canonicalizer evolution needs maintenance. B's soundness problem returns multiplied by projection consumers trusting components. This candid criticism applies: version 1 carries over-engineering risk. Several benefits land in projections and queries, outside this bead's commitment.

---

**Candidate D — Git-clean-tree hybrid (exact-state token per repository policy).**

*Position.* Adopt `reference_identity` wholesale: `(repository_id, store_path, scope, commit, graph_digest)`. Validity demands commit-currency plus digest-equality over clean scopes. Git landing remains policy, never implication.

*Mechanism sketch.* Commit reruns `issue`'s procedure under the lock for involved scopes. Steps: resolve HEAD (refusing shallow, `git.rs:27-33`); compare canonical bytes across commit/index/worktree; recompute repository id and digest; compare framed token component-wise like `verify_and_load`'s ordered mismatches. Repository-policy switches decide whether successful publication requests companion Git commits from humans/CI. Engine stays write-free, keeping decision 9 open.

*Preserves.* Total alignment with existing exact-state vocabulary. References and exports remain artifacts of committed graphs. Strongest audit story: a committed transaction pins *where* (commit) plus *what* (digest).

*Interleave annoyance.* It fails in one specific way: the commit component fires independent of graph movement. Any commit voids open tokens even in frozen scopes — README, Cargo.lock alike. Cure exists narrowly: `issue` checks only selected-scope canonical bytes (fact 5). Replicating narrowness demotes commit-mismatch to advisory metadata. Then D reduces to A plus provenance logging.

*Retry.* All parts revalidate means accept. Any part moved means named via ordered `Mismatched{field:…}`. Idempotent and diagnosable.

*Clone determinism.* Established mechanism (`reference_identity`, root-commit hashing). Inherits shallow-clone exclusion, costing ephemeral CI checkouts. A/B/C avoid that cost.

*Merge driver.* Landed merges move commit and digest together beyond either side's anticipation. Behaviour equals A's refuse/replan, compounded by unrelated commits mid-flight.

*What makes it wrong.* Strict form couples Provenance concurrency to unrelated repository cadence; false refusals track overall commit volume. Loose/advisory form becomes indistinguishable from A decorated with provenance. Residual value is an audit annotation, not a gate.

---

**Cross-candidate constants (costed once).**

- *Typed refusals.* All four need stale-class failures elevated from `anyhow` strings to an enum carrying `field/expected/actual`. Threaded through an SDK envelope owning no error variant today (`response.rs:18`); working-state writers migrate case by case. Extending `GraphReferenceError` versus minting a transaction-domain enum: naming decision reserved for reviewer.
- *Determinism doctrine.* Every viable candidate inherits `canonical.rs` discipline. None invents a second hasher.
- *Lock integration.* All gate recomputation inside `with_repository_publication`, relying on reentrancy. None alters lock order.
- *Merge correctness.* All treat post-merge state as moved for watched components. Finding 9 forces this regardless of candidate.
-¹ *Footnote (explicitly out of scope per brief):* the later refinement intersecting intervening transactions' written families against the planned change's affected set could certify many currently-refused commits safe, shrinking the interleave annoyance for B/C substantially — recorded so its absence is a known trade-off, not an oversight.

**Decisions reserved explicitly to the human reviewer.**

1. Choose among A (simplest, roughest edges), B (scoped; conditional on a closure audit never performed), C (most machinery, best locality, also serves projections), D (provenance-rich; refuses often when strict; becomes A plus logging when loose). No ranking offered.
2. Resolve the projection-boundary contradiction. Are fog and open Topics/Questions inside the write-precondition universe? Options visible: split write-time projection from reference projection; relocate fog out of `Requirement`; accept cross-actor replans as the price of one canonicalization. Docs gesture at family-scoped low-ceremony writes (`cli.md:285-287`).
3. If B or C favoured: authorize the closure-proof programme. Enumerate each operation's true read/write families; evidence exists they span edges/bindings/reviews; registry-check them; conservative wide default for unaudited operations.
4. Fix Git participation for v1. Either keep the engine commit-free (today's reality) with landing owned by humans/CI, or consciously open decision 9 for specific state classes, accepting shallow-CI constraints.
5. Set the refusal-machinery budget. New typed enum plus envelope error channel versus extending `GraphReferenceError`. Fix wire names (`base_revision` versus `expected_state_token`) and opacity.
6. Decide kernel scope. Does version 1 apply the precondition across state classes (working-state ops checked but unapproved, per docs' sketch)? Or does it ship graph-intent-only initially?
7. Confirm the accepted v1 trade-off. Unrelated working-state edits may force replan; remedy deferred to the footnote refinement.
