# Draft position: a relation is a field or an action record, never a canonical edge

Draft for the human disposal of `q_relations_first_class_or_properties`.
Written 2026-09-03 after the three stance artifacts, the relation map
(`docs/research/2026-09-03-relation-ontology-by-authoring-act.md`), and the
owner's reaction on the question thread. Under adversarial review. Not decided.

## Position

1. A relation is a field on the record that makes the claim. The record's
   type says whether the field is required or optional. A rule needs a
   requirement, so the field is required. A requirement cites sources, so the
   list is optional and may be empty.
2. Where the claim is an action by an actor, the relation is that action's
   record. The record names its targets, its actor, its date, and its own
   retirement. Bindings, reviews, assertions, and dispositions already work
   this way.
3. Reverse lookups are materialized in the projection and never written by
   hand. "Which requirements depend on this rule" is a query over the
   projection, not a stored fact.
4. There is no canonical edge. The edges shard is retired. The nine edge
   types become either a field on their owner or an action record.
5. One declaration per record kind names its fields, which fields are
   references, and the kind each reference points to. The writer, the merge
   validator, the gap policy, and the projection derive their behavior from
   that declaration. Nobody maintains a relation by hand.

## What each edge type becomes

| Edge type today | Becomes | Owner |
|---|---|---|
| references (requirement to source) | the existing citation list on the requirement | requirement |
| refines_into (requirement to requirement) | a required parent field on the child requirement, absent only on a root | child requirement |
| depends_on (requirement to requirement) | an optional list on the dependent requirement | dependent requirement |
| supersedes (any to any) | the existing superseded_by field on the older record | older record |
| needs (requirement to resolution) | dropped; it mirrors resolves | none |
| resolves (resolution to requirement) | a required field on the resolution | resolution |
| spawns (resolution to requirement) | an optional list on the resolution | resolution |
| produces (requirement or resolution to rule) | a required requirement field on the rule, plus an optional resolution field | rule |
| contradicts (requirement to requirement) | an action record: a review that names both requirements, its actor, and its evidence | the review |

## Rationale

- Every relation the type already requires is a field, and none has drifted.
  Every edge row lacks an author, a date, a status, and a label, and two
  double-written relations have drifted (79 citations against 76 edges, 97
  needs against 95 resolves).
- The half of the model built later (bindings, reviews, assertions,
  dispositions) already follows rules 1 and 2. The decision brings the
  original four kinds up to that standard.
- A fact with one owner merges per record. The single edges shard appears in
  45 of 67 state commits.
- Rust's ownership model and the typed declarations both assume one owner
  per fact. The edge is the one primitive that fights that.
- The projection derives everything from records (W2). A graph database
  later is a projection target that generates its edges from the same
  declarations.

## Costs

- The nine edge types are in the SDK wire protocol and the typed
  declarations produce them. Retiring them is a protocol change and a
  typed-spec change.
- Five relations have no authoring command today (refines_into, depends_on,
  contradicts, supersedes, spawns). Each needs a command or a declaration
  field, per the table above.
- The W4 relation vocabulary keeps its read-layer role. Its edge-row
  derivation becomes a projection materialization, which lands in W3.
- Migration: three steps from the record-owned artifact. Stop double writes.
  Add owner fields and backfill from the 612 edge rows. Delete the shard.

## Open

- Whether contradicts is a review or a new action record kind.
- Whether refines_into as a required parent field breaks any root
  requirement today, and how a root is marked.
- Whether the typed declaration surface can express every reference field
  without a new declaration kind.
