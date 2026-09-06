# Source refinement hierarchy proposal

*Graph design research for the source-shaping session. This document is a
proposal for Ben to review. It changes no canonical graph record.*

## The problem

The current anchor `req_source_refinement_for_product_work` states:

> Source refinement organizes source material into an evolving system
> description with the Provenance ontology.

It refines `req_create_an_end_to_end_product_research_to`. That parent states:

> Every rule traces back through its producing resolution and requirement to a
> motivating source, and the walk is a single query.

The parent statement is about rule-to-source traceability. Source refinement
is about people who structure informal material together. The `refines` edge
does not match the statements. The parent works only as a discovery program
root, because ten discovery requirements hang under it as one flat list.

`req_team_review_uses_repository_artifacts` is a sibling of the anchor under
that same parent. The comment and discussion Rules cite both the shared view
requirement and the team review requirement. The two areas belong together,
but no node states that today.

## Evidence in the graph

All cluster requirements cite the source
`source_provenance_product_discovery_2026_09_04`. These are the only
requirements that cite it:

| Requirement | Refines today | Decision that spawned it |
|---|---|---|
| `req_source_refinement_for_product_work` | `req_create_an_end_to_end_product_research_to` | `res_source_refinement_structures_evolving_intent` |
| `req_refinement_tracks_discussion_agenda` | `req_source_refinement_for_product_work` | `res_refinement_uses_visible_discussion_agenda` |
| `req_refinement_agent_uses_product_context` | `req_source_refinement_for_product_work` | `res_refinement_agent_leads_informed_shaping` |
| `req_refinement_shows_evolving_structure` | `req_source_refinement_for_product_work` | `res_refinement_exposes_evolving_structure` |
| `req_shared_view_edits_sources_and_records` | `req_refinement_shows_evolving_structure` | `res_shared_view_edits_sources_and_records` |
| `req_requirement_document_nests_record_lineage` | `req_refinement_shows_evolving_structure` | `res_requirement_document_follows_record_lineage` |
| `req_team_review_uses_repository_artifacts` | `req_create_an_end_to_end_product_research_to` | `res_team_review_starts_with_local_artifacts` |

Related records that stay as they are:

- `res_shared_view_reuses_discussion_and_lifecycle` covers
  `req_refinement_shows_evolving_structure`.
- Eight comment Rules (`rule_comment_*`, `rule_record_comment_*`,
  `rule_reply_threads_resolve_independently`,
  `rule_discussion_outcome_shows_record_change`) name both
  `req_refinement_shows_evolving_structure` and
  `req_team_review_uses_repository_artifacts` in `requirement_ids`. Rules need
  no Resolution producer, and these have none.
- `rule_shared_view_uses_provenance_type_names` covers the shared view
  requirement. `rule_record_depth_preserves_reading_width` covers the nested
  document requirement.
- `boundary_initial_review_needs_no_hosted_service` attaches to the team
  review requirement.
- Open questions `question_initial_repository_review_route` and
  `question_comment_created_record_default_parent` attach to the team review
  requirement. Open questions `question_source_edit_existing_citations` and
  `question_shared_view_mcp_app_host_support` attach to the anchor.
- Topics `topic_source_refinement_structure` and
  `topic_repository_review_workflow` anchor on the anchor requirement and the
  team review requirement.

## Proposed hierarchy

```
req_create_an_end_to_end_product_research_to        [resolved; unchanged]
└─ req_people_shape_and_review_intent               [NEW]
   ├─ req_source_refinement_for_product_work        [refines changed]
   │  ├─ req_refinement_tracks_discussion_agenda    [unchanged]
   │  ├─ req_refinement_agent_uses_product_context  [unchanged]
   │  └─ req_refinement_shows_evolving_structure    [unchanged]
   │     ├─ req_requirement_document_nests_record_lineage  [unchanged]
   │     └─ req_shared_view_edits_sources_and_records      [unchanged]
   └─ req_team_review_uses_repository_artifacts     [refines changed]
```

Read the tree as: the program root keeps the traceability walk; one new
grouping requirement carries the joint human work; source refinement keeps its
three facets and the shared view keeps its two facets; team review becomes the
review surface beside the shaping surface.

## The new grouping requirement

No existing node can parent this cluster. The closest candidates fail:

- `req_provide_a_durable_thread_and_message_dis` is about thread storage on
  every artifact type. It is engine capability at a lower altitude, not a
  collaboration surface.
- `req_provenance_shall_support_structured_mult` is about multi-agent ideation
  and proposals. Different work.
- `req_team_review_uses_repository_artifacts` is a sibling, not a parent. Its
  statement names review only.
- `req_canonical_active_thread` is one write-time property of threads.

The new node names one shared decision, so it passes the climb test: Ben
ratified that people work on system intent together with Provenance's own
records, with local artifacts first. Each child adds one surface for that
work.

- Proposed id: `req_people_shape_and_review_intent`
  (Ben picks the final id. An alternative is `req_collaborative_intent_surfaces`.)
- Proposed statement: "People shape and review the evolving system intent
  together with Provenance's records and repository artifacts."
- Proposed description: "Provenance's records are the medium for the joint
  work. A refinement session turns source material into Requirements,
  Resolutions and Rules, and a review reads and discusses those records
  without a hosted service. This requirement names that shared medium. It adds
  no behaviour of its own."
- Proposed status: `discovery`. The surfaces are still shaped.
- Proposed domain: `domain_shaping`.
- Proposed source reference: `source_provenance_product_discovery_2026_09_04`,
  with clauses for the source breakdown message
  (`thr_requirement_req_create_an_end_to_end_product_research_to_0/msg_1781475924590`),
  the local-first priority
  (`msg_06f4c7af4001Xh5voJML4OZJv6`), and the local review routes
  (`msg_1781475924604`). These clauses are evidence for the grouping. They do
  not parent it.

The ticket export idea stays out. The source floats it, no decision exists,
and the anchor's fog already holds it. Fog stays fog until it sharpens.

## Exact relation changes

| # | Record | Change |
|---|---|---|
| 1 | `req_people_shape_and_review_intent` | Create it. Set `refines` to `req_create_an_end_to_end_product_research_to`. Set status `discovery`, domain `domain_shaping`, one source reference. |
| 2 | `req_source_refinement_for_product_work` | Change `refines` from `req_create_an_end_to_end_product_research_to` to `req_people_shape_and_review_intent`. |
| 3 | `req_team_review_uses_repository_artifacts` | Change `refines` from `req_create_an_end_to_end_product_research_to` to `req_people_shape_and_review_intent`. |

Nothing else moves. Keep these fields exactly as they are:

- `spawned_by` on every cluster requirement. Example: the anchor keeps
  `spawned_by` = `res_source_refinement_structures_evolving_intent`, although
  that Resolution names the old parent in `requirement_ids`. The Resolution
  records where the decision was made. It is history, not placement.
- `origin_thread` and `origin_message` on every cluster requirement. Example:
  the anchor keeps its origin in
  `thr_requirement_req_create_an_end_to_end_product_research_to_0`. Birthplace
  is history, not placement.
- `requirement_ids` on every Rule and Resolution, including the dual coverage
  of the eight comment Rules.
- All five `refines` edges inside the source refinement subtree.
- Source references on cluster requirements. Source citations are evidence,
  never requirement parents.

## Placement rationale, area by area

**Source refinement.** The anchor keeps its statement, description, fog, and
origin. It changes only its parent. Its statement scopes the subtree: every
child names a source refinement session.

**Agenda.** `req_refinement_tracks_discussion_agenda` stays a direct child of
the anchor. Its statement names "a source refinement session", so the anchor
is its only correct parent.

**Agent guidance.** `req_refinement_agent_uses_product_context` stays a direct
child of the anchor. Its statement names "source refinement" directly. The
anchor and the agent guidance requirement describe the two roles in one
conversation, so they sit one level apart.

**Shared view and editing.** `req_refinement_shows_evolving_structure` stays a
child of the anchor. Its statement scopes the view to "during source
refinement". Its two children stay under it: the nested document names the
reading form, and `req_shared_view_edits_sources_and_records` names the
editing behaviour. Both decisions were made in the shared view thread of the
parent requirement, and their source clauses cite that thread.

**Discussion and review.** `req_team_review_uses_repository_artifacts` moves
under the new grouping requirement. Its boundary, its two open questions, its
topic, and its origin stay attached to it. The eight comment Rules keep their
dual `requirement_ids`, because record comments work on both the shared view
and the review surface. The engine-side discussion records stay where they
are: `req_provide_a_durable_thread_and_message_dis` keeps
`refines` = `req_create_an_end_to_end_product_research_to`, because thread
storage serves every artifact type, and `req_canonical_active_thread` keeps no
parent. Moving them is engine-wide cleanup, and this proposal does not do
cleanup.

**Comment-created records.** No new node. Three Rules already carry the
concept, and `question_comment_created_record_default_parent` is still open.
When Ben answers that question and the concept hardens, a child requirement
under the team review requirement can hold it. Until then the Rules and the
question are the correct home.

## Uncertain placements

- **Parent of the new grouping requirement.** This proposal keeps the path to
  the program root: the new node refines the old parent. The statement
  mismatch then sits one level up, between the collaboration layer and the
  traceability walk. The other option is no `refines` on the new node, like
  `req_canonical_active_thread`. That cuts the cluster off from the program
  root. A third option, re-organizing all ten children of the old parent into
  program areas, is a separate decision. Ben chooses.
- **Editing under the shared view.**
  `req_shared_view_edits_sources_and_records` could instead refine the anchor
  directly, because editing is not only about seeing. This proposal keeps it
  under `req_refinement_shows_evolving_structure`, because its decision
  thread, its source clause, and its sibling document requirement all sit
  there.
- **A session conduct grouping.** A node for "how a session runs" could group
  the agenda and agent guidance requirements. With three children under the
  anchor, that layer is speculative. This proposal does not mint it.
- **Domain on the anchor.** The anchor has no domain. Setting it to
  `domain_shaping`, with the new node, makes the wiki domain rollup through
  `req_wiki_domain_membership_follows_refinement` uniform. This is a record
  edit, not a relation change, and Ben decides.
- **`question_source_refinement_session_outcome`.** It is answered and
  attached to the old parent, but its answer became the anchor's statement.
  Moving it to the anchor is coherent, but question placement is out of scope
  here.

## Review checks

- Every id in this document is copied from
  `.provenance/state/scopes/default/` on this branch at commit `7a8e251`.
- The installed `provenance` CLI (0.2.2) reads schema version 1 only, so this
  review reads the canonical JSONL directly. A newer CLI can verify the tree
  with `provenance prime`.
- This proposal makes no canonical graph change. Ben reviews, then a shaping
  session lands the three relation changes with the provenance CLI.
