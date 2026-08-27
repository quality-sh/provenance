---
date: 2026-08-27
bead: provenance-0i8
epic: provenance-46p
stage: plan-pending-human-review
model: glm-5.3-flash-high
---

SUBJECT BEAD: provenance-0i8. Task text: Retire or formally park Thread and Message records. This is the Plan stage. The Structure stage sits at `docs/research/2026-08-27-qrspi-0i8-threads-messages-retirement.md`. Human review fixed the WHAT as full retirement, Structure Candidate 1, with its recommended sub-decisions: retire the origin back-reference fields with the family, land the preservation ledger first, delete nothing before that. This document fixes HOW. It adds no facts about the code without a checked citation. I re-opened every site I cite during this Plan pass. Line numbers below come from this worktree, not copied from the Structure doc.

=== APPROACH SUMMARY ===

Preserve first, delete second. W1 lands the preservation ledger: one ADR that quotes both thread Rule texts, records the shaping-session protocol vocabulary, and indexes the eight ledger items from Structure. W1 ships alone.

W2 removes the four `origin_thread`/`origin_message` field pairs on Source, Requirement, Resolution, and Rule. The export strip walk shrinks by the same amount in the same commit, so its strip-and-refuse pair stays atomic (`projection.rs`). W2 ships alone.

W3 deletes the record family end to end: model, pure logic, shard paths, store API, SQLite materialization, CLI verbs, prime seam, wiki field notes. Migrations 018 and 019 mirror the services trail: drop tables, then purge shards by runner hook. W3 and W4 share one change set.

W4 retires the two thread Rules, their requirement and resolution, and eight edges. Graph retirement moves with code deletion because stale-marker warnings fire when markers cite retired Rules (`handlers/coverage.rs`).

W5 sweeps format docs and fixtures, then runs the final gates: workspace tests, strict coverage scan, `provenance check`, wiki build.

Word count of this summary: 171.

=== WORKSTREAM BREAKDOWN ===

Execution order note: W1 lands first and independently. W2 lands second and independently. W3+W4 land together as one reviewable change set with ordered internal commits (family deletion, then graph retirement last). W5 trails. Rationale for the W3+W4 coupling sits in W4.

**W1 — Preservation ledger (Complexity: S)**

Goal: No valuable idea dies with the schema. All eight ledger items from Structure get durable homes before any deletion.

Touched files:
- New `docs/adr/0009-thread-message-retirement-ledger.md`. It quotes both active thread Rule texts verbatim from `.provenance/state/scopes/default/rules/rule.jsonl:5` and `rule.jsonl:138`. It records the race incident narrative pointer (`res_posting_reconciles_active_threads`, `.provenance/state/scopes/default/resolutions/res.jsonl:36`; raw transcript at `.provenance/state/scopes/default/threads/2026-07.jsonl:58`). It lists the CHART/HANDOFF/DECISION protocol vocabulary and points at example threads in git history. It notes the month-shard pattern, the per-message `ai_metadata` slot (`collaboration.rs:63-64`, column at `crates/provenance-store/migrations/004_threads_messages.sql:22`), and the privacy rationale codified at `crates/provenance-store/src/graph_reference/projection.rs:166-203`. If the reviewer answers Open Question 2 with "promote", it also fixes landing locations for the STE/homepage transcripts.
- `docs/shaping.md`: append the session communication protocol section if the reviewer approves that home.
- `docs/state-format.md`: one history line beside the current family list at `docs/state-format.md:119`, recording the month-sharded append-log pattern for future reuse. The generic multi-directory reader already exists at `crates/provenance-store/src/state_store/readers.rs:347-380`, so later reuse needs no new machinery.

Migration notes: none. Doc-only.

Test strategy: none beyond the repository's usual gates. A human reads the ADR next to rule.jsonl and confirms each quote matches character for character.

Rollout gate: W1 merges before any deletion commit exists on any branch. Reviewer sign-off on this merge equals Structure decision (b).

**W2 — Origin back-reference field removal (Complexity: L)**

Goal: All four canonical record kinds lose `origin_thread`/`origin_message`. The collaboration strip walk loses exactly those visits. Nothing dangles, nothing hides.

Touched files:
- Field definitions: `crates/provenance-core/src/model/artifacts.rs:264-275`, `:311-317`, `:394-400`, `:438-444`. Two blocks per kind, four kinds.
- Strip/refuse pair: `crates/provenance-store/src/graph_reference/projection.rs:179-202` — remove the eight visit lines for origin fields, keep the four `claimed_by`/`claimed_at` visits. The import half `validate_no_collaboration_fields` follows the same walk automatically (`projection.rs:124-136`); removing a visited field makes old pinned graphs carry an unknown key, which `deny_unknown_fields` refuses (`projection.rs:20-27`). Both halves flip at once by construction; the file's own comment documents this exact reverse move for services.
- Dangling-check arms: `crates/provenance-cli/src/handlers/check/references.rs:7-36` loses `check_origin_references`. Its four call sites go: `check/scope/core.rs:221-229` (sources), `:248-256` (requirements), `:355-364` (resolutions), `:365-373` (rules).
- CLI flags: `crates/provenance-cli/src/cli/knowledge.rs:31-33,57-59`; `crates/provenance-cli/src/cli/policy.rs:45-47,96-104`.
- Handlers: `handlers/sources.rs:23-40`; `handlers/requirements.rs:25-38`; `handlers/resolutions.rs:31-53`; `handlers/rules.rs:86-104` plus the `None` literals at `rules.rs:158-159`.
- Store plumbing: `state_store/writers.rs:23-24,44-45,69-70,96-97`; `state_store/typed_specs/reconcile.rs:67-68,171-172,271-272`.
- Tests: `cli_origin.rs` becomes a refusal test (unknown flag exits non-zero). Projection expectations flip in `graph_reference_errors.rs` and `graph_reference/projection/tests/collaboration.rs`. Model fixtures in `model/tests/artifacts.rs` lose the fields.
- Live data: the seventeen populated values (14 rules, 1 requirement at `req.jsonl:48`, 2 resolutions at `res.jsonl:74-75`) do not need hand-editing. Plain record structs carry no `deny_unknown_fields` on these four kinds, so old JSONL keys are ignored on read (inference, see Facts vs Inference). Each affected file loses the dead keys when its writer next rewrites it.

Migration notes: none. No SQLite columns involved; origin fields never reached the cache.

Test strategy: after removal, one probe test feeds a source line carrying `"origin_thread"` through plain load and through pinned-graph import. The first must parse clean (unknown key ignored); the second must refuse. This pins both halves.

Rollout gate: `cargo test --workspace` green, `provenance check` green on this repository (17 live references stop being checked in the same commit that removes their check sites), coverage scan strict clean. Ships as its own PR; no dependency on W3.

**W3 — Record family deletion plus migrations 018/019 (Complexity: L)**

Goal: Thread and Message cease to exist as code and as storage. The services trail is the template.

Touched files:
- Core model: delete `crates/provenance-core/src/model/collaboration.rs:1-65`. Remove its re-export `model.rs:17` and the names in `lib.rs:22-30`. Delete the logic module declared at `lib.rs:7`: `crates/provenance-core/src/threads.rs:1-292` including the property suite at `:51-292` and both `#[rule]` markers at `:10,:35`.
- Pure writers: delete `crates/provenance-store/src/state_store/thread_writers.rs:1-100` (`post_thread_message` at `:6-12`; hardcoded `created_at: 1` inside the Thread literal near `:50`; synthesized `max+1` message timestamps near `:60-67`).
- Shard paths: `shards.rs:83-95` — `threads_path` writes `threads/threads.jsonl`; `messages_path` hardcodes `threads/2026-07.jsonl` at `:94`. Both die. Readers: `state_store/readers.rs:306-354` (`read_message_shards`, `is_message_month_shard`) dies with them.
- Store API: `state_store.rs:244-249` (`list_threads`, `list_messages`); `PostMessageInput` at `inputs.rs:265-270`.
- Cache: materialization loops at `cache/materialize/collaboration_records.rs:10-30` (threads insert, messages insert); family entry at `cache/materialize.rs:57`; the whole prime seam — `cache/prime.rs:26,32,43-57,72,92` (`include_threads`, `PrimeThreadView`, render loop), `crates/provenance-cli/src/cli.rs:167` (flag), `handlers/mod.rs:150-158`, `handlers/prime.rs:14-26`.
- CLI verbs: `cli/shaping.rs:5-31` (`ThreadCommand::Post/List`), variant `cli.rs:139-142`, dispatch `handlers/mod.rs:137-140`, handler `handlers/thread.rs:1-44` (44 lines).
- Export/import: `handlers/export.rs:28-29,64-65`; `import/scope_writer.rs:11,54-59` and reconcile block `:84-109`; totals at `import.rs:53-54`.
- Wiki field notes: types `wiki/model.rs:199-220` plus page struct fields at `:246,278,331,354`; assembly `wiki/assemble.rs:68` and `assemble/evidence.rs:116-159` (`evidence_thread`, `threads_for`); page calls `assemble/pages/source.rs:43`, `pages/requirement.rs:24-26,62`, `pages/resolution.rs:77`, `pages/rule.rs:85`; render `render/field_notes.rs:11-41` and helper `render/labels.rs:155-159`; fixture builders `render/tests/fixtures.rs`, `assemble/tests/fixtures.rs`.
- Migration files: new `crates/provenance-store/migrations/018_drop_threads_messages.sql` mirroring `016_drop_rule_code_and_services.sql:1-11` in shape — drop `idx_messages_thread_order` (name from `004_threads_messages.sql:25`), drop `messages`, drop `idx_threads_parent_status` (name from `004:9`), drop `threads`. New `migrations/019_remove_threads_shards.sql` as a sentinel comment like `017_remove_services_shards.sql:1`.
- Runner wiring: constants beside `crates/provenance-store/src/migrations.rs:8`, include_strs beside `:26,43-45`, applied-id lists extended past `:178-189`, hook branch beside `:100-101`, purge function beside `remove_services_shards` at `:117-153`, purging `scopes/*/threads/*.jsonl`.
- Test replacements: delete `cli_check_threads.rs`, `cli_import_threads.rs`, `cli_thread_prime.rs`, and `cli_check.rs:261-288` (the August-shard test that only the reader could accept). Add migration tests beside the services ones at `migrations.rs:216-263`: 018 drops cleanly, 019 purges populated and absent shard dirs, cache primes after cleanup. Add one absence test: `provenance check` passes on a scope whose threads directory is gone.
- Fixture payload cleanup: `docs/research/assets/2026-08-07-wiki-homepage-scope-index/generate-scale-fixture.py:106` and `crates/provenance-cli/src/wiki/fixtures_scale.rs:121` drop their `"threads": []` entries once `GraphExport` loses the fields.

Migration notes:
- 004 built the tables (`004_threads_messages.sql:1-25`), wired at `migrations.rs:8,26`. No SQLite file exists in this worktree (`find *.db *.sqlite*` outside `.git` and `target` returns nothing), so table-drop has zero local victims today. Other machines may differ; 018 must be idempotent exactly like 016's `IF EXISTS` shape.
- Order is strict: graph retirement (W4) executes inside this same change set, last, never before the marker deletion.
- Live shard disposal runs through the 019 hook at the next migration cycle, not by a manual `rm`. Git already tracks both files (29 lines and 92 lines counted this session), so history keeps every byte. This posture awaits Open Question 1.

Test strategy: workspace tests plus three targeted additions above. Coverage scan sees neither markers nor implementations for the two retired ids after W4 lands.

Rollout gate: all merged-set commits keep compile, clippy, and tests green; after the full set, `cargo test --workspace && cargo clippy --workspace --all-targets` clean; `rg -n "post_thread_message|ThreadCommand|include_threads|MessageRole" crates packages` returns only unrelated hits.

**W4 — Graph self-reference retirement (Complexity: M)**

Goal: The graph stops legislating threads. Strict coverage never observes a retired Rule with a live marker.

Mechanic and why W3+W4 share one change set: `handlers/coverage/retired.rs:12-40` turns any scanner annotation citing a retired or non-active Rule into a warning, and `--strict` makes warnings fail (`AGENTS.md:104-108`). Retiring the records first would strand the two `#[rule]` markers until W3 landed. Deleting the markers first leaves unimplemented Rules, which is an ordinary derived state (`AGENTS.md:116-118`). So: family deletion commits first, graph retirement commits last, same set.

Touched state files:
- `.provenance/state/scopes/default/rules/rule.jsonl:5` and `:138` — mark `rule_canonical_thread` and `rule_thread_siblings_archived` retired through the ordinary status/retired mutation, not deletion, preserving the historical text quoted in ADR 0009.
- Edges naming the family: `.provenance/state/edges/edges-00.jsonl:103,104` (produces edges to both rules), `:308,309` (produces edges from `res_posting_reconciles_active_threads`), `:3` and `:533` (needs/resolves pair between `req_canonical_active_thread` and that resolution), `:437` (references edge onto `req_canonical_active_thread`). Eight lines total; endpoints must leave no dangling references or `provenance check` fails.
- Requirement and resolution feeding them: `requirements/req.jsonl:4` (`req_canonical_active_thread`), `resolutions/res.jsonl:36` — disposition awaits Open Question 3 (retire fully versus deprecate-and-keep as historical record).

Migration notes: state edits happen through normal store/editor flows, gated by `provenance check`. Import validation rejects malformed combinations (`import/scope_writer.rs:84-109` shows the strictness precedent), so round-trip export/import is part of the gate.

Test strategy: after retirement, run `provenance coverage scan --path . --scope default --validate-rules --strict`; expect exit zero and zero warnings naming either id. Export/import round-trip test covering the edited scope stays green.

Rollout gate: final commit of the merged set. Scan strict exit zero is the hard gate; a red scan blocks merge of the whole set.

**W5 — Format docs, fixtures sweep, final verification (Complexity: S)**

Goal: No documentation lies after the retirement.

Touched files:
- `docs/state-format.md:119` — remove Threads/Messages from the shipped JSONL family list.
- `docs/state-format.md:7` — remove "resolved thread status" from the preserved-fields sentence.
- No other repo doc mentions the family: verified silence across `README.md`, `CONTEXT.md`, `AGENTS.md`, `docs/cli.md`, `docs/cache.md`, `docs/release.md` (grep returned nothing this session).

Migration notes: none.

Test strategy: grep gate plus rerun of all W2/W3/W4 gates untouched.

Rollout gate: `rg -ni "thread" README.md CONTEXT.md AGENTS.md docs/*.md` returns hits only where prose legitimately discusses the retirement itself (ADR, this plan's successors, research docs).

=== OPEN QUESTIONS FOR HUMAN REVIEW ===

1. Disposal posture for the live shards: accept migration-driven deletion of `.provenance/state/scopes/default/threads/*.jsonl` from the working tree, with git history as the only remaining copy? Or archive-copy the two files into `docs/research/assets/` first? Recommendation: plain delete; history suffices; nothing in the ledger needs a second copy.
2. Should the STE/wiki-homepage shaping narratives become a static prose page under `docs/`, visible without git archaeology? Ledger item 3 proposes moving the vocabulary into `docs/shaping.md`. Recommendation: yes to the vocabulary section; the raw transcripts stay git-only.
3. Fate of `req_canonical_active_thread` and `res_posting_reconciles_active_threads`: retire both along with the two Rules, or mark them deprecated but keep them visible as historical record? Recommendation: retire fully; the ADR carries their story.
4. Flip window: if any reviewer prefers Structure Candidate 2 (park dormant behind ADR + deprecation + post-refusal) instead of approved removal, say so before implementation starts; the plan then stops after W1 and rebases onto the park bundle. Silence confirms removal.
5. Bead bookkeeping: workers never touch bd. Confirm that a human updates and closes provenance-0i8 after the implementation phases land, absorbing ledger item 8 (the July-shard defect record).

=== ACCEPTANCE CHECKLIST ===

Each promised outcome maps to one observable verification:

1. Record family gone from code — `rg -n "enum ThreadStatus|struct Thread\b|MessageRole|post_thread_message|PostMessageInput|ThreadCommand|include_threads" crates packages` returns zero hits outside git history and docs.
2. Origin back-references gone — `rg -n "origin_thread|origin_message|originThread|originMessage" crates packages` returns zero production hits; `crates/provenance-cli/tests/cli_origin.rs` asserts unknown-flag refusal; probe test proves plain readers tolerate old JSONL keys while pinned-graph import refuses them.
3. SQLite mirror gone for fresh caches — 018 test drops indexes and tables; priming a scope creates a cache with no `threads`/`messages` tables; `migrations.rs` applied-list ends past `019`.
4. Shards purged by migration, recoverable from history — after cache migration on this repository, `ls .provenance/state/scopes/default/threads` fails; `git show <pre-change-sha>:.provenance/state/scopes/default/threads/2026-07.jsonl` returns the 92 messages.
5. Check green — `provenance check` exits zero on this repository after all five workstreams.
6. Coverage stays honest — `provenance coverage scan --path . --scope default --validate-rules --strict` exits zero with no warning naming `rule_canonical_thread` or `rule_thread_siblings_archived`.
7. Privacy discipline intact — projection tests prove `claimed_by`/`claimed_at` still stripped on export and still refused on import; a graph carrying `originThread` is refused on import.
8. Wiki renders without field notes — wiki build green; rendered pages contain no "Discussion"/field-note sections; scale fixture has no threads entry.
9. Ledger durable — `docs/adr/0009-thread-message-retirement-ledger.md` exists, quotes both Rule texts verbatim, covers all eight ledger items, and predates the first deletion commit (verifiable by commit order).
10. SDK surface unchanged — TS package diff touches nothing (`packages/provenance/src/index.ts:437`, `src/protocol.ts:115` hold only generic error strings today); Rust SDK crate shows no thread symbols before or after.
11. Format docs truthful — the two `docs/state-format.md` edits land; no other shipped doc names the family.

=== OUT OF SCOPE RESTATED ===

- Executing any edit, implementation, commit, push, PR, or child issue from this document. This Plan stage produces exactly one markdown deliverable.
- The Candidate 2 park bundle (ADR + `#[deprecated]` + runtime post-refusal). Built only if Open Question 4 flips the decision.
- Fixing the hardcoded `messages_path` month shard (`shards.rs:94`). Under removal the defect dies with its owner; nobody repairs a feature headed for deletion.
- Convex-era cloud history migration. Local repository only.
- Storage-strategy redesign for programmable graph changes beyond the dependency check already recorded (one taxonomic mention at `docs/research/2026-08-27-programmable-graph-change-proposals.md:259`; the transaction kernel at `:258` needs no Thread/Message).
- Redesign of ideation or collaboration features. Reuse of the preserved patterns is a future, separate decision.
- Any update to the bead tracker. Standing law bars bd commands for workers.
- Reopening Structure decisions already disposed by human review.

=== FACTS VERSUS INFERENCE ===

Facts (verified against this worktree during the Plan pass):
- Family model in one file (`collaboration.rs:1-65`); re-exports at `model.rs:17` and `lib.rs:7,22-30`; logic and property suite in `threads.rs:1-292` with markers at `:10,:35`.
- Writer bug: `messages_path` hardcodes `threads/2026-07.jsonl` (`shards.rs:90-95`); reader accepts any `YYYY-MM.jsonl` (`readers.rs:343-353`).
- Store API, input type, writer timestamps: `state_store.rs:244-249`, `inputs.rs:265-270`, `thread_writers.rs` hardcode and synthesis sites as cited in W3.
- Migration 004 schema and index names; 016 and 017 precedent shapes; runner hook mechanics (`migrations.rs:100-153`) and applied-id lists (`:178-189`).
- Materialization loops (`collaboration_records.rs:10-30`) and prime seam (`prime.rs:26-92`, `cli.rs:167`, `handlers/mod.rs:150-158`, `prime handler :14-26`).
- Full origin-field touch inventory (this pass found more call sites than Structure listed: store writers, typed-spec reconcilers, handlers' `None` literals).
- Strip/refuse pairing and its documented reversibility (`projection.rs:166-203`, `:20-27`, `:124-136`).
- Check arms at `core.rs:221-229,248-256,355-373` plus dangling logic at `references.rs:7-36`; failure-mode tests at `cli_check.rs:231-288`.
- Export/import surfaces as cited in W3.
- Live data: 29 threads, 92 messages in one July shard, both git-tracked; 17 origin-referencing canonical records (14 rules, 1 requirement at `req.jsonl:48`, 2 resolutions at `res.jsonl:74-75`); 8 edge lines naming threads (`edges-00.jsonl:3,103,104,308,309,437,484,533`); both thread Rules at `rule.jsonl:5,138` with producing requirement at `req.jsonl:4` and resolution at `res.jsonl:36`.
- Stale-marker warning mechanism exists (`retired.rs:12-40`, filter at `coverage.rs:176`, strictness described at `AGENTS.md:104-108`).
- No `deny_unknown_fields` on the four canonical record structs; no SDK thread mirrors; documented format exposure only at `docs/state-format.md:7,119`; no local SQLite file.

Inference (flagged, each converted into a gate):
- That retiring graph records before marker deletion breaks the strict scan rests on reading `retired.rs` and `coverage.rs` together; nobody ran that experiment here. The W3+W4 shared change set makes the sequence safe regardless of the exact warning wording.
- That plain readers ignore removed origin keys rests on serde defaults and the absence of `deny_unknown_fields` on those structs; W2 carries an explicit probe test rather than trusting the reading.
- Zero-loss-under-removal assumes nobody rewrites git history after the shard purge.
- The stranding prediction that motivated Structure's ordering concern reduces to the same stale-marker mechanism; it does not depend on source-document existence checks, which remain unverified.
