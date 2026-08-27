---
date: 2026-08-27
bead: provenance-0i8
epic: provenance-46p
stage: structure-complete-awaiting-disposal
model: glm-5.3-flash-high
---

SUBJECT BEAD: provenance-0i8. Task text: Retire or formally park Thread and Message records. Epic: provenance-46p. The .beads tracker stores beads in Dolt (.beads/metadata.json:1-7). Standing law barred bd commands, so I could not read the bead body. This document uses the subject text from the launcher brief as given.

=== QUESTION ===

This decision covers the Thread and Message record family. Provenance removed the Services family before. That removal ran a SQLite drop migration, then a shard purge by the migration runner. Option one removes Threads and Messages the same way. Option two parks them dormant behind a durable, discoverable marker. Under either option, the useful ideas in the live thread data must move to durable places. That data spans about eight months. No valuable idea may die with the schema or stay held by it.

Sub-questions:
1. What is the full blast radius of a delete? Count the model types, the pure logic, store readers and writers, month-shard mechanics, the SQLite mirror across migration 004, materialization, check and validate arms, export/import plumbing, CLI verbs, wiki rendering, and origin back-references on Source/Requirement/Resolution/Rule. Which TypeScript or Rust SDK surfaces change?
2. What live Thread/Message rows exist in this repository right now? Can anyone recover them elsewhere (are they tracked by git)? What data does each option lose?
3. What does keep-but-dormant truly cost per year? Include the confirmed broken month-shard writer (hardcoded `threads/2026-07.jsonl`).
4. Does the near-term direction in docs/research/2026-08-27-programmable-graph-change-proposals.md need Threads/Messages? The coordinator believes no.
5. If we park the records, which marker form is durable and enforceable, rather than prose that rots?
6. Which self-referential graph artifacts ABOUT threads must retire or migrate in step? Two active Rules, one Requirement, at least one Resolution, eight edges, and wiki field-note sections qualify.

In scope: the disposition of the Thread/Message record family end-to-end across the Rust workspace. Also in scope: the fate of the `origin_thread`/`origin_message` back-reference fields on the four canonical record kinds. Also the fate of thread-governing graph records in `.provenance/state`. Also preservation routing for ideas that currently live only in these records. Also the marker and durability mechanism for either outcome.

Out of scope: executing any edit, plan, implementation, commit, push, PR, or child issue. QRSPI stops after Structure. Also out of scope: deciding the broader storage strategy of the programmable-graph-change proposal beyond checking dependency. Also out of scope: redesign of ideation/collaboration features. Also out of scope: migration of Convex-era cloud history. Also out of scope: updates to the bead tracker itself.

The answer must enable five reviewer decisions. (a) Remove now versus park dormant, with full knowledge of the live data. (b) Approval of the preservation-landing locations in the ledger below. (c) Disposal posture for the live shards. A migrating cleanup deletes files. Git retains their history. (d) If parking, ratification of a specific marker bundle and reopening criteria. (e) Whether the origin back-reference fields die with the referents or linger.

=== RESEARCH ===

Model layer.
One file holds the whole record family. File: crates/provenance-core/src/model/collaboration.rs:1-65. It defines ThreadStatus, MessageRole (+parse), ThreadParent {node_type, node_id}, Thread, and Message.
Re-exports sit at crates/provenance-core/src/model.rs:17 and crates/provenance-core/src/lib.rs:22-28. lib.rs:7 declares the logic module crates/provenance-core/src/threads.rs. In that module, choose_canonical_active_thread filters Active records and picks the winner (threads.rs:12-20 region). archive_non_canonical_siblings sets losers to Archived (threads.rs:38-47). The same file carries a large property-test suite (threads.rs:56-290).

Month-shard mechanics and the confirmed writer bug.
The writer paths are fixed. threads_path writes `threads/threads.jsonl`. messages_path always writes the hardcoded `threads/2026-07.jsonl`. See crates/provenance-store/src/shards.rs:83-95. The bug sits at :94.
The reader side is generic. read_message_shards lists the threads directory. is_message_month_shard accepts any `YYYY-MM.jsonl`. See crates/provenance-store/src/state_store/readers.rs:306-345. One test plants a hand-authored `2026-08.jsonl` that the writer can never produce, and check passes. Test: crates/provenance-cli/tests/cli_check.rs:261-288.
Verified consequence: new posts always append into the July shard. The monthly-sharding intent is broken. However, no message is ever lost or unreadable. Timestamp fidelity already decayed. Writers synthesize each message created_at as max(existing)+1 (thread_writers.rs:62-67). They hardcode thread created_at to 1 (thread_writers.rs:50).

SQLite mirror.
Migration 004 creates the `threads` and `messages` tables plus indexes. File: crates/provenance-store/migrations/004_threads_messages.sql:1-25. Constants and the apply loop wire it at crates/provenance-store/src/migrations.rs:8,26,58.
Materialization inserts every thread and message row each cycle. See crates/provenance-store/src/cache/materialize/collaboration_records.rs:11-26. The store API pair sits at crates/provenance-store/src/state_store.rs:244-248. PostMessageInput carries `parent`/`role` at crates/provenance-store/src/state_store/inputs.rs:267-268.
No SQLite database file exists in this worktree right now (`find *.db/*.sqlite` finds nothing outside `.git`). So today the table-dropping half of a drop migration has zero locally materialized victims.

Services removal precedent.
Model side: someone deleted the Service type and left only Domain in place. See crates/provenance-core/src/model/services.rs:1-15. The file kept its old name. Do not repeat that.
Cache side: migration 016 drops indexes and tables. File: crates/provenance-store/migrations/016_drop_rule_code_and_services.sql:1-11.
Shard side: migration 017 is a sentinel comment at migrations/017_remove_services_shards.sql:1. A runner hook joins it. The hook purges `scopes/*/services/*.jsonl` across all scopes. Hook site: migrations.rs:100-101 and remove_services_shards at migrations.rs:117-153. Tests cover it at migrations.rs:216-263.

CLI surface.
CLI verbs exist. ThreadCommand::Post and ::List sit at crates/provenance-cli/src/cli/shaping.rs:5-30. A command variant sits at cli.rs:139-141. Dispatch happens at handlers/mod.rs:137-139. Implementation sits at handlers/thread.rs:20-40.
Create verbs accept `--origin-thread`/`--origin-message`. Sources create reads them (cli/knowledge.rs:31; handlers/sources.rs:23-40). Requirements create reads them (knowledge.rs:57; handlers/requirements.rs:25-38). Resolutions create reads them (cli/policy.rs:45; handlers/resolutions.rs:31-53). Rules create reads them (policy.rs:96-99; handlers/rules.rs:86-104).

Check/validation arms.
check loads threads and messages. It indexes them as `"thread"`/`"message"` nodes. It checks ownership and cross-references. See handlers/check/scope/collaboration.rs:12-99.
check also runs dangling checks on `origin_thread`/`origin_message` for the four record kinds. Call sites: check/scope/core.rs:221-229, 248-255, 355-373. Logic site: check/references.rs:7-36. One test pins failure when referents are missing. Test: cli_check.rs:231-259. Its stderr contains `"origin_thread thread thread_missing"`.
This bears on every removal plan. If threads vanish while back-references stay populated, `provenance check` fails on this very repository. See the live data below.

Export/import plumbing.
Scope export includes threads. Sites: handlers/export.rs:28,64. Import writes them back. Import reconciles one active thread per parent and rejects duplicates. Sites: handlers/import/scope_writer.rs:11,54-56,84-109. Import totals count them. Site: handlers/import.rs:53.

Back-reference fields and their privacy treatment.
All four canonical kinds carry optional `origin_thread`/`origin_message` fields. Serde maps camelCase aliases and skips absent fields. Site: crates/provenance-core/src/model/artifacts.rs:264-275. Twin blocks sit at 311-317, 394-400, and 438-444.
Pinned graph-reference exports strip these fields. A walk visits every collaboration field under rule `rule_export_strips_collaboration`. The recorded reason: they name people and conversations. See crates/provenance-store/src/graph_reference/projection.rs:164-203 (visit list 191-202). Import symmetrically refuses graphs that still carry them. Refusal sites: projection.rs:22-27,124-127. Every field decision must shrink this walk consistently. The paired strip/refuse design at 171-174 makes partial edits impossible to hide.

Wiki rendering consumes the records (contradicts "no shipped surface consumes them" in one direction).
The wiki renders threads as field notes. FieldNote/EvidenceThread types sit at crates/provenance-cli/src/wiki/model.rs:199-220. Assembly pulls them from state onto source/resolution/rule pages. Assembly site: wiki/assemble.rs:68. Source pages call threads_for at pages/source.rs:43. Requirement pages borrow their resolving resolutions' threads at pages/requirement.rs:24-26. Resolution pages at pages/resolution.rs:77. Rule pages at pages/rule.rs:85. Render helpers sit at labels.rs:155-159 and field_notes.rs:39-41.
The TS SDK carries zero Thread/Message mirrors. It carries only generic error.message strings (packages/provenance/src/index.ts:437, packages/provenance/src/protocol.ts:115). A workspace-wide package search found nothing else. The Rust SDK crate has none either. An rg search over crates/provenance-sdk/src returned empty.

Live data inventory (this repository).
`.provenance/state/scopes/default/threads/threads.jsonl` holds **29 threads**. `2026-07.jsonl` holds **92 messages**. July is the *only* message shard. This matches the hardcoded writer. Both files are git-**tracked**. `git ls-files` lists both under .provenance. Deletion would remove the working-tree copy only. History keeps the content recoverable.
The content is not noise. It includes multi-persona ideation review verdicts (msg lines 52-59). It includes a documented concurrent-posting race and its manual repair (line 58). It includes broker security backfill evidence chains (lines 21-51). It includes the whole STE/wiki-homepage shaping-history protocol. Those CHART/BLOCKED-ON-HUMAN/REACTION/CONFIRMATION/HANDOFF/CORRECTION/DECISION messages sit at lines 60-76, 77-92. Several carry mid-August 2026 dates (`msg_17814759244xx-5xx`). People used the convenience path weeks ago.
Canonical records point at threads. **17 records** carry populated `origin_thread`. 14 rules sit in rules/rule.jsonl (for example lines 109-123). 1 requirement sits at requirements/req.jsonl:48. 2 resolutions sit at resolutions/res.jsonl:74-75.
The graph also legislates about threads. Two **active** rules exist: `rule_canonical_thread` and `rule_thread_siblings_archived`. Their statements pin to choose_canonical_active_thread/archive_non_canonical_siblings. Their source_document fields point at crates/provenance-core/src/threads.rs. Locations: rule.jsonl:5 and :138. Rule :138 also notes a then-open import-validation gap. A requirement feeds them: req_canonical_active_thread. A resolution feeds them: res_posting_reconciles_active_threads. Edges connect them (edges/edges-00.jsonl:103,308; 8 edge lines mention threads). Deleting threads.rs without retiring these through the normal coverage flow strands strict-rule coverage and check.

Near-term direction dependency check.
docs/research/2026-08-27-programmable-graph-change-proposals.md mentions threads exactly once. That mention is taxonomic. Line 259 lists "shaping threads" as an example item in the Working-state class. The document never mentions messages, comments, or conversations anywhere else (rg: only hits are unrelated). Its Planned-Change transaction kernel builds on Sources/Requirements/Rules/Resolutions/Boundaries/Domains/edges (:258). The same document treats SQLite/wiki as rebuildable projections (:263). Coordinator belief is **confirmed**: nothing mechanical in that direction needs Thread/Message records. The one taxonomy word is editorial.

Docs/format exposure and unresolved anchors.
On-disk format documentation lists threads/messages among JSONL record families. Site: docs/state-format.md:119. It lists "resolved thread status" among optional preserved fields. Site: docs/state-format.md:7. docs/cli.md, README.md, CONTEXT.md, and AGENTS.md contain no thread mentions (rg: empty). An undocumented verb shipped by accident of momentum.
Missing anchor: docs/research/2026-08-27-data-model-and-erd.md does not exist at current main. docs/research holds four other files. A repo-wide search finds nothing matching figure-3/"discussion attach"/erd-named strings. Its claims (figure 3, discussion-attachments paragraph, open questions) could not be verified. Treat them as launcher-context-only.
Leftover fixture payload: fixture-scale tooling still fabricates `"threads": []` payloads. Evidence: docs/research/assets/2026-08-07-wiki-homepage-scope-index/generate-scale-fixture.py:106 and wiki/fixtures_scale.rs:121.

Evidence-split checklist.
Repository facts (cited above): the model/logic file contents and locations. The hardcoded July writer path. The month-agnostic reader. The migration 004 schema plus the 016/017 precedent machinery. The CLI verbs and origin flags. The check arms and their failure modes. The export/import and projection stripping pair. Wiki consumption. The TS/Rust SDK absence. 29 threads plus 92 messages in exactly one July shard, tracked by git. 17 origin-referencing canonical records. 2 active thread rules with a producing requirement/resolution and 8 edges. One taxonomic mention in the programmable-proposals doc. The absent ERD anchor doc. No local SQLite file.
Inference (mine, flagged): the no-data-loss reading of the writer bug rests on reading writer and reader together. The August-activity reading of the timestamps rests on id/time patterns. The living-reliance reading of the mid-August messages rests on recency, not proof of irreplaceability. The stranding prediction assumes the strict-coverage gate treats a deleted source like a removed source. Nobody verified that mechanically. The zero-loss-under-park and history-only-loss-under-remove postures assume nobody rewrites git history.

=== STRUCTURE ===

Candidate 1 — Remove fully, following the services trail (016+017 pattern).

Position: Thread and Message come from before the local move. No external consumer uses them. The forward direction does not need them. Delete the family completely. Preserve the ideas in documents first. Destroy nothing before that. Retire the self-referential graph artifacts in step.

Mechanism sketch (interfaces and invariants only):
- Sequence contract: land the Preservation Ledger (below) first. Retire `rule_canonical_thread`/`rule_thread_siblings_archived` next, together with their requirement/resolution. Use the ordinary coverage/retirement flow. Strict coverage must never observe a deleted `source_document`. Delete code only after that.
- Deletion unit boundaries: `model/collaboration.rs` plus its three re-export sites. The `core::threads` module. `StateStore::{post_thread_message, list_threads, list_messages}` and `PostMessageInput`. `shards::{threads_path, messages_path}`. The thread arms in check: collaboration.rs Records, index registration, and the scope/core.rs origin-reference calls. Remove those origin-reference calls if and only if the origin fields also go. The check invariant couples the two decisions. clap `ThreadCommand` plus dispatch. Wiki `FieldNote`/`EvidenceThread` and render helpers. The prime context `include_threads` seam.
- Migrations: add `018_drop_threads_messages.sql` mirroring 016 (drop idx_messages_thread_order, drop messages, drop idx_threads_parent_status, drop threads). Add a `019_remove_threads_shards.sql` sentinel with a runner hook mirroring `remove_services_shards`. It purges `scopes/*/threads/*.jsonl`.
- Origin back-references: recommendation to remove the four field pairs simultaneously and shrink the `rule_export_strips_collaboration` walk by the same amount. The strip/refuse pairing at projection.rs:171-174 makes this atomic. Add or remove one visited field, and both halves flip at once. One verification gate remains. Old JSONL lines will carry now-unknown keys. Confirm the plain state readers tolerate them. Strict `deny_unknown_fields` was observed only in the graph-reference path (projection.rs:22). Treat this tolerance as a gate for the candidate. Do not ship it as an assumption.
- Wiki continuity: field notes disappear from rendered pages. If the reviewer wants the STE/homepage shaping narratives visible, promote them from git-only data to a static prose page. Fold that promotion into the ledger step. Do not continue a data path for it.

Existing behaviour it must preserve: checks stay green after rule retirement and field removal. Export/import round-trips of remaining families stay untouched. Pinned-export stripping stays complete for `claimed_by`/`claimed_at`. The walk survives, smaller. TS and Rust SDK stay unaffected. No surface was found.

Tradeoffs: maximal schema economy. The July-shard trap and the undocumented verb die permanently. The sweep touches ~20 files plus 2 migrations. Real regression surface sits in check/wiki/export. In-product browsability of historical conversations ends immediately. If the recent heavy use of `thread post` reflects an ongoing habit, workflow breakage is certain and visible.

What would make it wrong: discovering an automated pipeline still feeding `thread post` for handoffs (the August messages suggest this habit). Shipping removal before the two Rules retire (coverage strands, the epic fails its own gates). Deleting shards in the migration while the ledger step lagged (git history then becomes the only record of DECISION/CONFIRMATION content).

Candidate 2 — Park dormant behind a formal, enforced marker.

Position: the records carry real, recently-used value. Their behaviour passes validation today. Freeze them instead of deleting them. Mark dormancy somewhere machine-enforceable. Prose alone rots.

Marker form, chosen and justified: a three-layer bundle.
(i) An ADR in docs/adr/. It names bead provenance-0i8. It states parked-since. It states reopening criteria (example trigger: a designed agent-discussion surface lands). It owns the July-shard defect as a sealed consequence.
(ii) Rust `#[deprecated]` attributes on the exported types/APIs and on the `ThreadCommand` enum. Any new internal caller trips a warning-to-be-denied at compile time.
(iii) A runtime refusal arm at the `Command::Thread` dispatch. It returns "thread records are parked. See ADR-XXXX and provenance-0i8." This freezes the broken writer by refusing posts instead of fixing them.
Rejected alternative: comments/deprecation alone. Prose rot is demonstrably easy here (model/services.rs kept the old filename after the type died, model/services.rs:1-15). A runtime refusal is the only marker that also protects the live data ledger. It blocks further mutation. Misuse then fails loudly instead of silently.

Mechanism sketch: everything stays. Model, logic, shard paths, migration 004 tables, materialization, check arms, wiki rendering, export/import all stay. Changes follow. The dispatch arm refuses `post`. `List` stays (read-only and harmless). The `--include-threads` flag on prime stays (already default-off, so the parked family costs nothing unless asked for). Do not fix the hardcoded messages_path. Document the hardcode as sealed. Fixing it (rolling month shards) would build features on a parked feature. That works against the goal. Cross-link the ADR from the two thread Rules' descriptions. Graph check then keeps humans oriented.

Existing behaviour it must preserve: literally everything above minus new posts. The 17 origin references stay resolvable, so check stays green. The two thread Rules keep their live implementations and verifications. Wiki field notes keep rendering the 92 stored messages. Readers keep accepting hand-placed future month shards (harmless).

Tradeoffs: zero data risk and zero compatibility risk. The diff takes about a day. But the full carrying cost persists indefinitely. It includes two SQLite tables written every materialization cycle, ~20 touchpoint files, an undocumented-but-shipped verb pair, wiki branches, and test suites. Every new mechanism in the programmable-direction must still enumerate this family and decide about it. Park markers historically decay unless enforced. Hence layer (iii).

What would make it wrong: a team appetite for schema hygiene that the marker bundle merely delays. SDK redesign proceeding family-by-family, where a dead-but-present family forces special cases in every new surface. Or a post refusal arriving without the ADR, which leaves users holding a dead verb and no explanation.

PRESERVATION LEDGER
1. The exactly-one-active-thread reconciliation design. Oldest active thread wins. Ties break by lower id. Writes archive sibling threads at write time. The already-landed Rule texts carry it: `rule_canonical_thread` and `rule_thread_siblings_archived` (state files rule.jsonl:5,138). Quote those texts verbatim into the retirement/park ADR. Under Candidate 1 the Rules retire with archived text. Under Candidate 2 nothing moves.
2. The concurrency-race incident record. Two active threads appeared for one parent. A manual merge repaired it. Narrative: line 58 of 2026-07.jsonl. The referenced decision is already canonicalized as `res_posting_reconciles_active_threads` (referenced from edges-00.jsonl:308). The ledger points the ADR at both. The raw message stays in git even after deletion.
3. The shaping-session communication protocol. Vocabulary: CHART, BLOCKED-ON-HUMAN, REACTION, CONFIRMATION, HANDOFF, CORRECTION, DECISION, with land-frontier bookkeeping. It currently lives implicit in messages 60-92 of 2026-07.jsonl. Move it into the operational skills/docs that orchestrate such sessions (agents learn the practice there today). The ADR records the vocabulary and points at example threads in git.
4. Concrete DECISION auditable transcripts backing approved STE and homepage Resolutions. After graduation those facts already live canonically at requirements/req.jsonl:48, resolutions/res.jsonl:74-75, and fourteen citing rules. The transcript tails become historical record referenced by git. Note them in the ADR appendix list.
5. The month-sharded append log design. Layout: a directory of `YYYY-MM.jsonl` files. Readers roll across them. Record it as a pattern note in docs/state-format.md history beside its current mention (:119). Later reuse is possible if ideation/audit records outgrow single shards. Generic multi-file directory reading already exists at readers.rs:347-369.
6. The per-message AI metadata passthrough slot (`ai_metadata`, collaboration.rs:63-64; cache column at 004_threads_messages.sql:22). Note it in the ADR as superseded for now. Current assertion/disposition payload columns cover the audit-slot need (materialize/collaboration_records.rs:45-76).
7. The origin attribution intuition (which conversation birthed this artifact). Part already survives: Resolutions carry `made_by`/`approved_by`/`inputs` (res.jsonl:74). The ADR records the privacy rationale codified at projection.rs:164-174. If Candidate 1 wins, add one line per affected record id (the 17 above). Each line states where its rationale now lives.
8. The July-shard defect itself and the diagnosis method (writer/reader asymmetry proof). It lives permanently in bead provenance-0i8. A human updates the bead either way. It also lives in the ADR, whatever the outcome.

Decisions left explicitly to the human reviewer:
1. Candidate selection. Full removal copies the 016 trail into a new 018/019 pair. Formal dormancy bundles ADR + deprecation + post-refusal. Weigh three facts: active August use, wiki consumption, and the confirmed direction-independence.
2. If removal: approve the order-of-operations sign-off. Land the ledger and retire the two thread Rules through normal flow BEFORE code deletion and the shard purge.
3. Removal disposal posture: accept 017-style deletion of live shards from the working tree (git history then holds the sole remaining copy) versus an archival copy step first.
4. Fate of the four `origin_thread`/`origin_message` field pairs: retire them with the family (recommended: check stays coherent and the collaboration-strip philosophy holds) versus retain them as inert provenance notes with dangling-tolerance carved out of check. The latter risks reopening the hole provenance exists to close.
5. If dormancy: ratify the three-layer marker bundle and its reopening criteria. Confirm two details. `thread post` should hard-fail, not warn. `messages_path` should stay broken-but-sealed, not fixed.
6. Decide whether the undocumented `ThreadCommand` verb (and `--include-threads`) is acceptable long-term as a parked public CLI surface, or must disappear regardless of the candidate chosen.
