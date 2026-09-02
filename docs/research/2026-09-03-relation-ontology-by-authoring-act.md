# Relation ontology by authoring act

Evidence for `q_relations_first_class_or_properties` (topic `topic_relation_shapes`). A map, not a proposal.
Read at `cde10bc` (main, PR 174). Counts are from `.provenance/state` in this checkout unless a tournament artifact is cited.
The three stance artifacts on `origin/1wh-shape-tournament` under `docs/proposals/` hold the argued cases; their counts are reused, not their arguments.

## 1. The table

One row per relation. "Act" is the exact command or function that creates the relation today. "Owner" is the record that act authors.
Class: **C** = containment (the type needs it), **A** = authored claim (a statement the author makes), **X** = act-relation (the relation is itself a record with metadata).
Storage: edge = row in `edges/edges-00.jsonl`; fk = id field on the from-side; emb = list or struct embedded in the from-side; own = its own record family.

| # | Relation | From | To | Authoring act today | Owner | Card. | Class | Lifecycle today | Recomputable | Storage | Drift or duality today |
|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | references | source | requirement | `requirements source-ref add` -> `write_source_reference` (writers.rs:131); `sdk apply` -> typed_specs/relationships.rs `reconcile`; raw `edges create` | requirement | many, opt | A | `edges delete`; typed reconciler deletes stale managed rows; no command removes a `source_refs` entry | yes, from row 16 | edge | 79 `source_refs` vs 76 edges; 3 pairs field-only (all `src_annotation_format_spec`); 0 edge-only; 0 of 612 edges carry `label` |
| 2 | refines_into | requirement | requirement | `edges create --type refines_into` only | none | many, opt | A | `edges delete` | no | edge | 19 rows, 19 distinct children; no create command writes it |
| 3 | depends_on | requirement | requirement | `edges create` only | none | many, opt | A | `edges delete` | no | edge | 0 rows |
| 4 | contradicts | requirement | requirement | `edges create` only | none | many, opt | A (symmetric finding) | `edges delete`; treated resolved by a `supersedes` edge or a shared resolution (gaps/contradiction.rs:33) | no | edge | 1 row |
| 5 | supersedes | requirement | requirement | `edges create` only | none | opt | A | `edges delete` | no | edge | 0 rows; requirement supersession has no other home |
| 6 | needs | requirement | resolution | `resolutions create --requirement-id` -> `write_resolution` (rule_writers.rs:71) writes needs and resolves together; raw `edges create` | resolution | opt-one at authoring | C (soft) | `edges delete`; `Resolution` keeps no `requirement_id` | yes, from row 7 | edge | 97 needs vs 95 resolves; 2 needs-only pairs, both on `req_rust_requirements_as_code_authoring` |
| 7 | resolves | resolution | requirement | same act as row 6 (rule_writers.rs:79) | resolution | opt-one at authoring; gap `OrphanResolution` wants >=1 (frontier.rs:54) | C (soft) | `edges delete` | yes, from row 6 | edge | 95 rows; 0 orphan resolutions |
| 8 | spawns | resolution | requirement | `edges create` only | none | many, opt | A | `edges delete` | no | edge | 1 row; shaping.md "Landing fan-out" names it but no landing act writes it |
| 9 | produces | requirement or resolution | rule | `rules create --requirement-id/--resolution-id` -> `write_rule` (rule_writers.rs:156,166); `sdk apply` (requirement side only); raw `edges create` | rule | opt at authoring; gap `OrphanRule` wants a requirement producer (`RuleProducer::REQUIRED`, graph_query.rs:20) | C (soft) | `edges delete`; typed reconciler deletes stale managed requirement->rule rows | no; `Rule` stores nothing | edge | 323 rows (183 req, 140 res); 0 rules lack a requirement producer; 31 lack a resolution producer |
| 10 | boundary_constrains | boundary | requirement | `boundaries create --requirement-id` -> `write_boundary` (shaping_writers.rs:23) | boundary | req-one | C | none; no update or delete for boundaries | n/a | fk | 3 rows |
| 11 | topic_shapes | topic | requirement | `topics create --requirement-id` -> `write_topic` | topic | req-one | C | none | n/a | fk | 4 rows |
| 12 | question_belongs_to_topic | question | topic | `questions create --topic-id` -> `write_question` | question | req-one | C | none | n/a | fk | 7 rows |
| 13 | question_refines | question | requirement | derived: `write_question` copies `topic.requirement_id` (shaping_writers.rs:147); no flag sets it | question | req-one | C (denormalised copy of 11+12) | none; nothing re-syncs it | yes, through the topic | fk | 7 of 7 agree with their topic |
| 14 | question_settled_by | question | resolution | `questions answer --resolution-id`, `questions update --resolution-id`, `questions create --resolution-id` | question | opt-one | A | overwrite via `questions update`; never cleared | no | fk | 3 of 7 set; 2 of 5 answered questions name no resolution |
| 15 | requirement_in_domain | requirement | domain | `requirements create --domain-id` -> `write_requirement` (writers.rs:73) | requirement | opt-one; gap `MissingDomainId` wants it | C (soft) | none; no command changes it after creation | n/a | fk | 67 of 68 set |
| 16 | requirement_cites_source | requirement | source | same act as row 1; `sdk apply` adds entries with `clause: None` and never removes one (reconcile.rs:202) | requirement | many, opt; gap `MissingSourceRefs` wants >=1 | A | append only | no; clause lives here only | emb | 79 entries on 62 requirements; 6 requirements have none; duality declared in `same_fact_as` |
| 17 | topic_links | topic | source/req/res/rule | `topics create --links-json` | topic | many, opt | A | none after creation | no | emb | 0 entries |
| 18 | question_links | question | source/req/res/rule | `questions create/update --links-json` (whole list replaced) | question | many, opt | A | replace list | no | emb | 0 entries |
| 19 | source_superseded_by | source | source | `sources create --superseded-by` (on the record being created); otherwise hand edit | source | opt-one | A | none; no `sources update` | no | fk | 0 of 52; `create_source` does not check the target exists |
| 20 | resolution_superseded_by | resolution | resolution | `resolutions create --superseded-by`; otherwise hand edit | resolution | opt-one | A | none; no `resolutions update` | no | fk | 1 of 93 (`res_convex_chosen...`, status `draft`); the one resolution with status `superseded` (`res_rule_is_the_function`) has no `superseded_by`; both directions disagree |
| 21 | boundary_cites_source | boundary | source | `boundaries create --source-id --source-clause` | boundary | opt-one | A | none | no | emb struct | 1 of 3 |
| 22 | thread parent | thread | source/req/res/rule/topic/question | `thread post --parent-type --parent-id` -> `write_thread_message` mints the thread on first post (thread_writers.rs:59) | thread | req-one | C | siblings archived by canonical choice; no delete | n/a | emb struct `ThreadParent` | 29 threads (15 req, 7 res, 6 rule, 1 question); writer checks kind only, not existence (thread_writers.rs:23-30) |
| 23 | message in thread | message | thread | same act; writer picks or creates the thread | message | req-one | C | none | n/a | fk | 92 messages, 0 dangling |
| 24 | origin_thread / origin_message | source/req/res/rule | thread / message | `<kind> create --origin-thread --origin-message` | the created record | opt-one | A (provenance pointer) | none | no | fk | req 1, res 2, rule 14, source 0; not checked at create; `check_origin_references` checks later |
| 25 | contribution target | contribution | source/req/res/rule/topic/question/domain | `contributions create --target-type --target-id`; `swarm-backtrace land` | contribution | req-one | C | replace until an assertion cites it, then frozen | n/a | emb `IdeationTarget` | 14 rows; writer does not check the target exists; `check_ideation_target` does |
| 26 | synthesis packet target | packet | same | `synthesis-packets create --target-type --target-id` | packet | req-one | C | as row 25 | n/a | emb | 3 rows |
| 27 | proposal target | proposal | same | `proposals create --target-type --target-id` | proposal | req-one | C | immutable once written | n/a | emb | 85 rows (80 source, 4 question, 1 resolution); 0 missing targets |
| 28 | proposal cites sources | proposal | source | `proposals create --source-id ...` | proposal | many, opt | A | immutable | no | emb `source_ids` | 85 entries, all resolve |
| 29 | proposal builds_on | proposal | assertion | `proposals create --builds-on` | proposal | many, opt | A (lineage) | immutable; acyclic check (lineage_validation.rs) | no | emb | 0 in use |
| 30 | proposal duplicate_of / superseded_by | proposal | proposal | none for modern rows: `validate_proposal_intrinsic` refuses them (lifecycle.rs:102); legacy rows only | disposition, by rule | opt-one | X (verdict) | n/a | meant to derive from a disposition, but `DispositionRecord` has no such field | fk | 0 in use; no path can express a duplicate today |
| 31 | assertion -> proposal | assertion | proposal | `proposals assert --proposal-id` -> `write_assertion`; `proposals create --assertion-id --synthesis-packet-id` -> `create_asserted_proposal` | assertion | req-one; one assertion per proposal | X (carries `supporting_claim_ids`) | immutable; refused after a disposition | n/a | own `ideation/assertions.jsonl` | 0 direct rows; 76 legacy `accepted` proposals predate the lifecycle |
| 32 | assertion -> synthesis packet | assertion | packet | same acts | assertion | req-one | X | immutable; packet frozen once cited | n/a | own | packet must exist and qualify the proposal (assertion_validation.rs:31) |
| 33 | disposition -> proposal | disposition | proposal | `dispositions create --proposal-id` -> `write_disposition`; gate `validate_disposition_write_gate` (proposal_writers.rs:350) | disposition | req-one; exactly one per proposal | X (actor, rationale, decision) | immutable | n/a | own `ideation/dispositions.jsonl` | 4 rows (1 accepted, 3 rejected); 76 legacy rows in `promotion_decisions.jsonl` |
| 34 | disposition -> canonical artifact | disposition | source/req/res/rule | `dispositions create --canonical-artifact-type/--id` | disposition | opt-one | X | immutable | n/a | emb | 2 of 4 (both resolutions); existence checked in scope (canonical_artifacts.rs:53) |
| 35 | implementation binding | binding | rule | `sdk apply` with `implementedBy` -> implementation_bindings.rs `reconcile`; `#[rule]` markers are read by `coverage scan` and never written (operations/sites.rs) | binding | req-one rule; one active binding per rule | X (`declared_by`, `retired`, file, symbol) | retire in place on omission or owner change | n/a | own `implementations/binding.jsonl` | 0 rows in live state |
| 36 | verification binding | binding | rule | `sdk begin-verification` -> `begin_verification` -> `materialize_verification_binding` (verification_runs.rs:70); `#[verifies]` markers read at scan, never written | binding | req-one rule; id = owner+rule+key | X (`declared_by`, method, `retired`) | retire in place when a key is re-pointed (verification_bindings.rs:80) | n/a | own `verifications/binding.jsonl` | 0 rows in live state |
| 37 | requirement review | review | rule and requirement | raised by `sdk apply` when a typed requirement statement changes (typed_specs.rs `raise_requirement_reviews`), one per rule found through `produces` rows; cleared by `sdk begin-verification` | review | req-one each | X (field, before, after, `changed_at`, `cleared_by_run`) | clear in place; never deleted | n/a | own `requirements/review.jsonl` | 0 rows in live state; rule set depends on row 9 |
| 38 | declared_by / declaration_address | source/req/rule | integration owner | `sdk apply` only (reconcile.rs:56,162,261); adoption via `adopt_unowned` | the record | opt-one | X (ownership) | retire on omission from the spec (typed_specs/lifecycle.rs) | n/a | fk pair | 0 of 287 records owned in live state |
| 39 | rule cites source document | rule | text | `rules create --source-document --source-section` | rule | opt | A (free text, not an id) | none | no | text fields | 131 of 167 rules; invisible to every traversal and gap |

## 2. What the table shows

1. The type system already forces every containment in rows 10 to 13 and 22 to 27: the child carries a required id field and `serde` refuses the row without it. All of them are fields or embedded structs; none is an edge row. The two relations the human's example names, rule needs a requirement (row 9) and resolution resolves a requirement (rows 6 and 7), are the opposite: optional inputs on the create command that fan out into edge rows, with the obligation enforced only afterwards by the gap policy.
2. Every act-relation (rows 31 to 38) already has identity, provenance, and a lifecycle: a derived id, an actor or `declared_by`, and retire or clear in place. Each has its own family. The 612 edge rows have none of that: no author, no date, no status, zero labels, and `edges delete` as the only lifecycle.
3. Three authored claims are written twice by one act: citation (rows 1 and 16, 79 vs 76, 3 drifted), needs and resolves (rows 6 and 7, 97 vs 95, 2 drifted), and the question's requirement copied from its topic (row 13, 7 of 7 agree). Supersession is stored once but disagrees with status in both directions (row 20).
4. Five declared relations have no authoring act at all beyond raw `edges create`: refines_into, depends_on, contradicts, supersedes, spawns (rows 2 to 5 and 8). Setting `superseded_by`, `domain_id`, or topic links on an existing record has no command either (rows 15, 17, 19, 20). A modern proposal cannot be marked a duplicate by any path (row 30).
5. `sdk apply` is the one act that writes several relations as a byproduct of authoring one document: `source_refs`, `references`, `produces`, implementation bindings, reviews, and `declared_by` in one call. It is also the act that keeps the citation duality alive, because `relationships.rs` writes the edge and `reconcile.rs` writes the field.

## 3. Required by the type in the model, not enforced by a writer or validator

- Rule needs a requirement. `CreateRuleInput.requirement_id: Option` (state_store/inputs.rs); `write_rule` checks existence only when given (rule_writers.rs:110). Reported later as `OrphanRule` (cache/gaps/frontier.rs:65-73, graph_query.rs:236). 0 orphans today.
- Resolution resolves a requirement. `CreateResolutionInput.requirement_id: Option`; `write_resolution` (rule_writers.rs:30). Reported as `OrphanResolution` (frontier.rs:54). 0 today.
- Requirement in a domain. Optional at `write_requirement` (writers.rs:73). Reported as `MissingDomainId` (frontier.rs:11-13). 1 of 68 missing.
- Requirement cites a source. Nothing at write. Reported as `MissingSourceRefs` (frontier.rs:21, graph_query.rs:255). 6 of 68 have none.
- Answered question settled by a resolution. `write_question_answer` accepts `None` (shaping_writers.rs:280-285). Not even a gap. 2 of 5 answered questions have none.
- Thread parent exists. `write_thread_message` checks the kind only (thread_writers.rs:23-30). Caught later by `add_thread_refs` (cache/gaps/dangling.rs:143).
- Ideation target exists. No check in `write_contribution`, `write_synthesis_packet`, or `write_proposal_card`. Caught by `check_ideation_target` at `provenance check` (handlers/check/references.rs:59).
- `superseded_by` and `origin_*` targets exist. `create_source` and `write_resolution` do not check. Caught by `check/scope/core.rs:210,344` and `dangling.rs:40,58`.
- Merge. `ShardFamily` recognises edges, requirements, rules, and landings only (merge/validation.rs:34-47). Every fk and embedded relation on sources, resolutions, boundaries, topics, questions, threads, and bindings merges unchecked.

## 4. Unsupported speculation and uncertainty

Speculation, marked as such:
- The 3 field-only citations (row 1) look like hand edits from before `add_source_reference` existed. I did not trace their commits.
- The 2 needs-only pairs (row 6) look like a deleted `resolves` edge, since the writer always emits both. I did not trace them.
- The typed SDK and MCP surfaces may reach writers through paths this grep did not cover. The CLI map in column 5 is from `crates/provenance-cli/src/handlers`.

Uncertainty: low on the table, medium on the classification.
Every count came from `jq` over this checkout or from the cited artifact. Every act came from reading the writer.
The C/A/X class is a reading of intent, not a fact in the code; rows 6, 7, 9, and 15 are marked "soft" because the model makes them optional while a gap policy or a doc says they are needed.
Row 39 is included because it is a citation the graph cannot see; whether it counts as a relation is the human's call.
