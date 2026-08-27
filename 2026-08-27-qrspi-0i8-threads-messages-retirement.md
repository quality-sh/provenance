---
date: 2026-08-27
bead: provenance-0i8
epic: provenance-46p
stage: structure-complete-awaiting-disposal
model: glm-5.3-flash-high
---

SUBJECT BEAD: provenance-0i8 — Retire or formally park Thread and Message records (epic provenance-46p). Bead body itself lives in a Dolt-backed tracker (.beads/metadata.json:1-7) and could not be opened: standing law forbids running bd/provenance-tracker commands, so the subject text is taken from the launcher brief as given.

=== QUESTION ===

Should Provenance retire the Thread and Message record family outright, following the removal trail already blazed for Services (SQLite drop migration + migration-runner shard purge), or formally park it dormant behind a durable, discoverable marker — and in either case, where do the worthwhile ideas embedded in twelve years of… (eight months of) live thread data get preserved durably, so nothing valuable dies with (or remains hostage to) dead schema?

Sub-questions:
1. What is the complete blast radius if Threads/Messages are deleted — model, pure logic, store readers/writers, month-shard mechanics, SQLite mirror (migration 004 lifecycle), materialization, check/validate arms, export/import plumbing, CLI verbs, wiki rendering, origin back-references on Source/Requirement/Resolution/Rule, TypeScript/Rust SDK surfaces?
2. What live Thread/Message rows exist in this repository right now, are they recoverable elsewhere (git-tracked?), and what is the honest data-loss posture under each candidate?
3. What is the true annual cost of keep-but-dormant, including the confirmed broken month-shard writer (hardcoded `threads/2026-07.jsonl`)?
4. Does the near-term direction in docs/research/2026-08-27-programmable-graph-change-proposals.md need Threads/Messages (coordinator belief: no)?
5. If kept dormant, what concrete marker form is durable and enforceable rather than prose that rots?
6. Which self-referential graph artifacts ABOUT threads (two active Rules, one Requirement, at least one Resolution, eight edges, wiki field-note sections) must be retired or migrated in step?

In-scope: disposition of the Thread/Message record family end-to-end across the Rust workspace; fate of `origin_thread`/`origin_message` back-reference fields on the four canonical record kinds; fate of thread-governing graph records in `.provenance/state`; preservation routing for ideas currently embodied only in these records; the marker/durability mechanism for either outcome.

Out-of-scope: executing any edit, plan, implementation, commit, push, PR, or child issue (QRSPI stops here); deciding the broader storage strategy of the programmable-graph-change proposal beyond checking dependency; redesigning ideation/collaboration features; migrating Convex-era cloud history; updating the bead tracker itself.

Decisions this answer must enable the human reviewer to make: (a) remove-now versus park-dormant, with eyes open on live data; (b) approval of the preservation-landing locations in the ledger below; (c) disposal posture for the live shards (migrating cleanup deletes files; git retains history); (d) if parking, ratification of a specific marker bundle and reopening criteria; (e) whether the origin back-reference fields die with the referents or linger.

=== RESEARCH ===

Model layer.
- The entire family is one small file: `ThreadStatus`, `MessageRole` (+parse), `ThreadParent {node_type, node_id}`, `Thread`, `Message` — crates/provenance-core/src/model/collaboration.rs:1-65. Re-exported at crates/provenance-core/src/model.rs:17 and crates/provenance-core/src/lib.rs:22-28; pure logic module is crates/provenance-core/src/threads.rs (declared lib.rs:7): `choose_canonical_active_thread` filters Active and picks winner (threads.rs:12-20 region), `archive_non_canonical_siblings` mutates losers to Archived (threads.rs:38-47), with an extensive property-test suite in the same file (threads.rs:56-290).

Month-shard mechanics and the confirmed writer bug.
- Writer paths: `threads_path` → `threads/threads.jsonl`; `messages_path` → **hardcoded** `threads/2026-07.jsonl` — crates/provenance-store/src/shards.rs:83-95 (bug at :94).
- Reader side is generic: `read_message_shards` enumerates the threads directory and accepts any `YYYY-MM.jsonl` via `is_message_month_shard` — crates/provenance-store/src/state_store/readers.rs:306-345. A test proves check passes with messages planted in a *manually authored* `2026-08.jsonl` the writer can never produce — crates/provenance-cli/tests/cli_check.rs:261-288.
- Consequence (verified, contradicting the "roll-forward" framing slightly): new posts always append into July's shard forever — the monthly-sharding intent is broken but **no messages are ever lost or unreadable**. Also note timestamp fidelity already decayed: message `created_at` is synthesized as `max(existing)+1` (thread_writers.rs:62-67) and thread `created_at` is hardcoded `1` (thread_writers.rs:50) on the convenience path.

SQLite mirror.
- Migration 004 creates `threads` and `messages` tables plus indexes — crates/provenance-store/migrations/004_threads_messages.sql:1-25; wired by id constants and the apply loop — crates/provenance-store/src/migrations.rs:8,26,58.
- Materialization inserts every thread/message row each cycle — crates/provenance-store/src/cache/materialize/collaboration_records.rs:11-26. `list_threads`/`list_messages` store API — crates/provenance-store/src/state_store.rs:244-248; `PostMessageInput` carries `parent`/`role` — crates/provenance-store/src/state_store/inputs.rs:267-268.
- No SQLite database file exists in this worktree right now (`find *.db/*.sqlite` empty outside `.git`), so the drop migration's table-dropping half has zero locally materialized victims today.

Services removal precedent (the trail to follow).
- Model-side: the `Service` type was simply deleted, leaving services.rs containing only `Domain` — crates/provenance-core/src/model/services.rs:1-15 (a filename-corpse worth renaming this time).
- Cache-side: 016 drops indexes/tables — crates/provenance-store/migrations/016_drop_rule_code_and_services.sql:1-11.
- Shard-side: 017 is a sentinel comment — migrations/017_remove_services_shards.sql:1 — plus a runner hook that purges `scopes/*/services/*.jsonl` across all scopes: migrations.rs:100-101 and `remove_services_shards` migrations.rs:117-153, tested at migrations.rs:216-263.

CLI surface.
- Verbs exist: `ThreadCommand::Post` and `::List` — crates/provenance-cli/src/cli/shaping.rs:5-30; command variant at cli.rs:139-141, dispatched handlers/mod.rs:137-139, implemented handlers/thread.rs:20-40.
- Create verbs accept `--origin-thread`/`--origin-message`: sources create (cli/knowledge.rs:31; handlers/sources.rs:23-40), requirements create (knowledge.rs:57; handlers/requirements.rs:25-38), resolutions create (cli/policy.rs:45; handlers/resolutions.rs:31-53), rules create (policy.rs:96-99; handlers/rules.rs:86-104).

Check/validation arms.
- Threads/messages are loaded, indexed as `"thread"`/`"message"` nodes, ownership-checked, and cross-referenced — handlers/check/scope/collaboration.rs:12-99.
- `origin_thread`/`origin_message` on the four record kinds are dangling-checked via `check_origin_references` — check/scope/core.rs:221-229, 248-255, 355-373 and check/references.rs:7-36. A test pins that missing referents FAIL check — cli_check.rs:231-259 (stderr contains `"origin_thread thread thread_missing"`). This is load-bearing for any removal: deleting threads while the back-references remain populated breaks `provenance check` on this very repo (see live data below).

Export/import plumbing.
- Scope export includes threads — handlers/export.rs:28,64; import writes them back and reconciles one-active-per-parent, rejecting duplicates — handlers/import/scope_writer.rs:11,54-56,84-109; counted in import totals — handlers/import.rs:53.

Back-reference fields and their privacy treatment.
- Optional `origin_thread`/`origin_message` sit on all four canonical kinds with camelCase aliases and `skip_serializing_if` — crates/provenance-core/src/model/artifacts.rs:264-275 (and twin blocks at 311-317, 394-400, 438-444).
- Pinned graph-reference exports strip them by walking every collaboration field under rule `rule_export_strips_collaboration`, because "they name people and conversations" — crates/provenance-store/src/graph_reference/projection.rs:164-203 (visit list 191-202); import symmetrically refuses graphs still carrying them — projection.rs:22-27,124-127. Any field decision here must shrink this walk consistently (the paired strip/refuse design at 171-174 makes partial edits impossible to hide).

Wiki: an actual shipped consumer (contradicts "no shipped surface consumes them" in one direction).
- Wiki models render threads as field notes: `FieldNote`/`EvidenceThread` — crates/provenance-cli/src/wiki/model.rs:199-220; assembled from state onto source/resolution/rule pages, with requirement pages borrowing their resolving resolutions' threads — wiki/assemble.rs:68, pages/source.rs:43, pages/requirement.rs:24-26, pages/resolution.rs:77, pages/rule.rs:85; rendered via labels.rs:155-159 and field_notes.rs:39-41. The TS SDK, however, contains zero Thread/Message mirrors — only generic `error.message` strings (packages/provenance/src/index.ts:437, packages/provenance/src/protocol.ts:115; workspace-wide package search otherwise empty). The Rust SDK crate likewise has none (rg over crates/provenance-sdk/src: empty).

Live data inventory (this repository).
- `.provenance/state/scopes/default/threads/threads.jsonl`: **29 threads**; `2026-07.jsonl`: **92 messages**, and July is the *only* message shard — consistent with the hardcoded writer. Both files are git-**tracked** (`git ls-files` lists both under .provenance), i.e., version-controlled, recoverable from history even after deletion from the working tree.
- Content is not noise. It includes: multi-persona ideation review verdicts (msg lines 52-59), a documented concurrent-posting race with manual repair (line 58), broker security backfill evidence chains (lines 21-51), and the entire STE/wiki-homepage shaping-history protocol — CHART/BLOCKED-ON-HUMAN/REACTION/CONFIRMATION/HANDOFF/CORRECTION/DECISION messages (lines 60-76, 77-92), several dated mid-August 2026 (`msg_17814759244xx-5xx`), i.e., the convenience path was in active use weeks ago.
- Canonical records referencing threads: **17 records** carry populated `origin_thread` — 14 rules (rules/rule.jsonl, e.g., lines 109-123), 1 requirement (requirements/req.jsonl:48), 2 resolutions (resolutions/res.jsonl:74-75).
- The graph legislates about threads: **active** `rule_canonical_thread` and `rule_thread_siblings_archived` pin their statements to `choose_canonical_active_thread`/`archive_non_canonical_siblings` with `source_document` pointing at crates/provenance-core/src/threads.rs (rule.jsonl:5 and :138, including an explicit note of a then-open import-validation gap), fed by requirement `req_canonical_active_thread` and resolution `res_posting_reconciles_active_threads` (edges/edges-00.jsonl:103,308; 8 edge lines mention threads). Deleting threads.rs without retiring these through the normal coverage flow will strand strict-rule coverage and check.

Near-term direction dependency check.
- docs/research/2026-08-27-programmable-graph-change-proposals.md mentions threads exactly once, taxonomically — "shaping threads" as an example item in the Working-state class — line 259; it never mentions messages, comments, or conversations anywhere else (rg: only hits are unrelated). Its Planned-Change transaction kernel builds on Sources/Requirements/Rules/Resolutions/Boundaries/Domains/edges (:258) and explicitly treats SQLite/wiki as rebuildable projections (:263). Coordinator belief is **confirmed**: nothing mechanical in that direction needs Thread/Message records; the single taxonomy word is editorial.

Docs/format exposure and anchors that did not resolve.
- On-disk format documentation lists threads/messages among JSONL record families — docs/state-format.md:119, and "resolved thread status" among optional preserved fields at :7. docs/cli.md, README.md, CONTEXT.md, AGENTS.md contain no thread mentions (rg: empty) — an undocumented verb shipped by accident of momentum.
- **Missing anchor**: docs/research/2026-08-27-data-model-and-erd.md does not exist at current main (docs/research holds four other files; repo-wide search for figure-3/"discussion attach"/erd-named strings finds nothing). Its claims (figure 3, discussion-attachments paragraph, open questions) could not be verified and must be treated as launcher-context-only.
- Stale-anachronism note: fixture-scale tooling still fabricates `"threads": []` payloads — docs/research/assets/2026-08-07-wiki-homepage-scope-index/generate-scale-fixture.py:106 and wiki/fixtures_scale.rs:121.

Evidence-split checklist.
Repository facts (cited above): model/logic file contents and locations; hardcoded July writer path; month-agnostic reader; migration 004 schema + 016/017 precedent machinery; CLI verbs and origin flags; check arms and their failure modes; export/import and projection stripping pair; wiki consumption; TS/Rust SDK absence; 29 threads + 92 messages in exactly one July shard, git-tracked; 17 origin-referencing canonical records; 2 active thread rules with a producing requirement/resolution and 8 edges; single taxonomic mention in the programmable-proposals doc; absent ERD anchor doc; no local SQLite file.
Inference (mine, flagged): that "no data loss" holds for the writer bug rests on reading writer+reader together; that the augmentation-era timestamps mean July-heavy traffic was actually August activity is inference from id/time patterns; that the mid-August messages indicate living reliance on `thread post` is inference from recency, not proof of irreplaceability; that rule/coverage stranding follows on threads.rs deletion assumes the strict-coverage gate treats deletion like removal of the cited source (unverified mechanically); the data-loss-under-park-is-zero and under-remove-is-git-history-only postures assume nobody rewrites git history.

=== STRUCTURE ===

Candidate 1 — Remove fully, following the services trail (016+017 pattern).

Position: Thread and Message are a pre-move fossil with no external consumer and no place in the forward direction; delete the family completely, preserving ideas in documents *before* anything is destroyed, and retire the self-referential graph artifacts in step.

Mechanism sketch (interfaces and invariants only):
- Sequence contract: land the Preservation Ledger (below) and retire `rule_canonical_thread`/`rule_thread_siblings_archived` (+ their requirement/resolution) through the ordinary coverage/retirement flow FIRST, so strict coverage never observes a deleted `source_document`. Only then delete code.
- Deletion unit boundaries: `model/collaboration.rs` and its three re-export sites; `core::threads` module; `StateStore::{post_thread_message, list_threads, list_messages}` and `PostMessageInput`; `shards::{threads_path, messages_path}`; thread arms in check (collaboration.rs Records, index registration, scope/core.rs origin-reference calls *if and only if* the origin fields are also removed — the two decisions are coupled by the check invariant); clap `ThreadCommand` + dispatch; wiki `FieldNote`/`EvidenceThread` and render helpers; `prime` context's `include_threads` seam.
- Migrations: `018_drop_threads_messages.sql` mirroring 016 (drop idx_messages_thread_order, drop messages, drop idx_threads_parent_status, drop threads), plus a `019_remove_threads_shards.sql` sentinel with a runner hook mirroring `remove_services_shards` but purging `scopes/*/threads/*.jsonl`.
- Origin back-references: recommended to remove the four field pairs simultaneously and shrink the `rule_export_strips_collaboration` walk accordingly (the strip/refuse pairing at projection.rs:171-174 makes this atomic — adding/removing a visited field flips both halves at once). Old JSONL lines carrying now-unknown keys must be confirmed tolerated by the plain state readers (strict `deny_unknown_fields` was observed only in the graph-reference path, projection.rs:22) — this tolerance is a verification gate for the candidate, not an assumption to ship on.
- Wiki continuity: field notes disappear from rendered pages; if the reviewer wants the STE/homepage shaping narratives visible, they graduate from git-only data to a static prose page as part of the ledger step, not as a continued data path.

Existing behaviour it must preserve: checks green after rule retirement and field removal; export/import round-trip of remaining families untouched; pinned-export stripping still complete for `claimed_by`/`claimed_at` (the walk survives shrunk); TS and Rust SDK unaffected (no surface found).

Tradeoffs: maximal schema economy; permanently kills the July-shard trap and the undocumented verb; ~20 files + 2 migrations in one sweep with real regression surface in check/wiki/export; loses in-product browsability of historical conversations immediately; if the recent heavy use of `thread post` reflects an ongoing habit, workflow breakage is certain and visible.

What would make it wrong: discovering any automated pipeline still feeding `thread post` for handoffs (the August messages suggest this habit); shipping removal before the two Rules are retired (coverage strands, epic fails its own gates); deleting shards in the migration while the ledger step lagged, turning "recoverable from git" into the only record of DECISION/CONFIRMATION content.

Candidate 2 — Park dormant behind a formal, enforced marker.

Position: the records carry real, recently-used value and currently validated behaviour; freeze rather than amputate. Dormancy must be marked somewhere machine-enforceable, not in prose alone.

Marker form, chosen and justified: a three-layer bundle — (i) an ADR in docs/adr/ that names bead provenance-0i8, states parked-since, reopening criteria (e.g., a designed agent-discussion surface lands), and explicitly owns the July-shard defect as a *sealed* consequence; (ii) Rust `#[deprecated]` attributes on the exported types/APIs and the `ThreadCommand` enum so any new internal caller trips a warning-to-be-denied at compile time; (iii) a runtime refusal arm at the `Command::Thread` dispatch returning "thread records are parked; see ADR-XXXX and provenance-0i8" — freezing the broken writer by refusing posts instead of fixing them. Comments/deprecation alone were rejected because prose rot is demonstrably easy here (cf. services.rs retaining its corpse name, model/services.rs:1-15); a runtime refusal is the only marker that also protects the ledger of live data from further mutation, converting a quiet latency into a loud boundary.

Mechanism sketch: everything stays — model, logic, shard paths, migration 004 tables, materialization, check arms, wiki rendering, export/import. Changes: dispatch arm refuses `post` (List stays, it is read-only and harmless); `--include-threads` on prime remains, already default-off, so the dormant family costs nothing unless asked for; `messages_path`'s hardcode is *documented* as sealed rather than fixed, since fixing it (rolling month shards) would be feature work on a parked feature — anti-goal. ADR cross-linked from the two thread Rules' descriptions so graph check keeps humans oriented.

Existing behaviour it must preserve: literally all of it, minus new posts; the 17 origin references stay resolvable (check stays green); the two thread Rules keep their live implementations and verifications; wiki field notes keep rendering the 92 stored messages; readers keep accepting future month shards should anyone hand-place them (harmless).

Tradeoffs: zero data and compatibility risk; a day-sized diff; but the full carrying cost (two SQLite tables written every materialization cycle, ~20 touchpoint files, an undocumented-but-shipped verb pair, wiki branches, test suites) persists indefinitely, every new mechanism in the programmable-direction must still enumerate (and decide about) this family, and park markers historically decay unless enforced — hence layer (iii).

What would make it wrong: if the team's actual appetite is schema hygiene and the marker bundle just buys procrastination; if SDK redesign work proceeds family-by-family and a zombie family forces special cases in every new surface; or if `post` refusal arrives without the ADR, leaving users with a dead verb and no explanation.

PRESERVATION LEDGER
1. Exactly-one-active-thread reconciliation design (oldest-active wins, id tiebreak; write-time archiving of siblings) — survives as its already-landed Rule texts `rule_canonical_thread` and `rule_thread_siblings_archived` (state files rule.jsonl:5,138), quoted verbatim into the retirement/park ADR; on Candidate 1 the Rules retire with their text archived there; on Candidate 2 no move is needed.
2. The concurrency-race incident record (dual active threads from one parent, manual merge repair) — narrative body of line 58 of 2026-07.jsonl; referenced decision already canonicalized as `res_posting_reconciles_active_threads` (referenced from edges-00.jsonl:308); ledger points the ADR at both; raw message stays in git even post-deletion.
3. The shaping-session communication protocol (CHART / BLOCKED-ON-HUMAN / REACTION / CONFIRMATION / HANDOFF / CORRECTION / DECISION vocabulary with land-frontier bookkeeping) — currently implicit in messages 60-92 of 2026-07.jsonl; moves into the operational skills/docs that orchestrate such sessions (the same place agents learn the ritual today), with the ADR recording the vocabulary and pointing at example threads in git.
4. Concrete DECISION auditable transcripts backing approved STE and homepage Resolutions — after graduation those facts already live canonically in requirements/req.jsonl:48, resolutions/res.jsonl:74-75 and fourteen citing rules; the transcript tails become git-referenced archaeology noted in the ADR appendix list.
5. Month-sharded append log design (directory of `YYYY-MM.jsonl`, reader-rolled) — recorded as a pattern note in docs/state-format.md history adjacent to its current mention (:119); reusable later if ideation/audit records outgrow single shards (multi-file directory reading already exists generically at readers.rs:347-369).
6. Per-message AI metadata passthrough slot (`ai_metadata`, collaboration.rs:63-64; cache column in 004_threads_messages.sql:22) — noted in the ADR as a superseded-for-now idea; today's assertion/disposition payload columns (materialize/collaboration_records.rs:45-76) cover the audit-slot need.
7. Origin attribution intuition (which conversation birthed this artifact) — survives partially already: Resolutions carry `made_by`/`approved_by`/`inputs` (res.jsonl:74); the ADR records the privacy rationale codified at projection.rs:164-174, and, if Candidate 1 is chosen, one line per affected record id (the 17 above) stating where its rationale now lives.
8. The July-shard defect itself and the diagnosis method (writer/reader asymmetry proof) — lives permanently in bead provenance-0i8 (human updates the bead either way) and in the ADR, whatever the outcome.

Decisions left explicitly to the human reviewer:
1. Candidate selection: full removal (trail of 016→own-018/019) versus formal dormancy (ADR + deprecation + post-refusal bundle) — with the weigh-in facts being active August use, wiki consumption, and the confirmed direction-independence.
2. If removal: order-of-operations sign-off — ledger landed and the two thread Rules retired through normal flow *before* code deletion and shard purge.
3. Removal disposal posture: accept 017-style deletion of live shards from the working tree (git history as sole remaining copy) versus an archival copy step first.
4. Fate of the four `origin_thread`/`origin_message` field pairs: retire with the family (recommended; keeps `check` coherent and matches the collaboration-strip philosophy) versus retain as inert provenance notes with dangling-tolerance carved out of check — the latter risks re-opening the very hole provenance exists to close.
5. If dormancy: ratify the three-layer marker bundle and its reopening criteria; confirm `thread post` should hard-fail rather than warn, and `messages_path` stays broken-but-sealed rather than fixed.
6. Whether the undocumented `ThreadCommand` verb (and `--include-threads`) is acceptable long-term as a parked public CLI surface, or must disappear regardless of the candidate chosen.
