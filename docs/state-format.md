# State Format

`.provenance/state/` is the canonical store. Records are newline-delimited JSON with stable string `id` fields, `schema_version`, and deterministic ordering by primary key inside each shard.

Scopes live in `manifest.json`; shard paths derive from scope IDs. Cache files and volatile fields are forbidden in state shards.

Schema version `2` includes the local graph fields plus imported/cloud review metadata. Optional fields are omitted when absent, but preserved when present: domain grouping for root requirements, requirement descriptions and source references, source references/clauses/effective/review dates/commit pins/supersession lists, draft/review statuses, resolution context/enforcement/confidence/input references/actor approval/supersession lists, resolved thread status, rule name/severity/status and source-document citations, proposal confidence, and material-claim confidence.

Relations between records are fields on the record that makes the claim. No
shard stores a relation row; every reverse lookup, gap, walk, and projection
row derives from the field. Lists are sorted by id on write and hold no
duplicates; `refines`, `depends_on`, and `supersedes` refuse a cycle at write.

| Kind | Field | Target | Required | Shape |
|---|---|---|---|---|
| requirement | `domain_id` | domain | optional | single |
| requirement | `source_refs[].source_id` (+`clause`), named `cites` | source | optional | list |
| requirement | `refines` | requirement | optional | single |
| requirement | `depends_on` | requirement | optional | list |
| requirement | `supersedes` | requirement | optional | list |
| requirement | `spawned_by` | resolution | optional | single |
| resolution | `requirement_ids` | requirement | required, non-empty | list |
| resolution | `supersedes` | resolution | optional | list |
| rule | `requirement_ids` | requirement | required, non-empty | list |
| rule | `resolution_ids` | resolution | optional | list |
| source | `supersedes` | source | optional | list |
| topic | `requirement_id` | requirement | required | single |
| boundary | `requirement_id` | requirement | required | single |
| boundary | `source_ref.source_id`, named `cites` | source | optional | single |
| question | `topic_id`, `requirement_id` | topic, requirement | required | single |
| question | `resolution_id` | resolution | optional | single |
| question | `contradicts` | requirement | optional | single |

A contradiction is a question: `topic_id` and `requirement_id` name one side,
`contradicts` the other. It is settled when the question carries a
`resolution_id` or either requirement lists the other in `supersedes`. A
requirement with no `refines` is a root. The newer of two sources or two
resolutions lists the older in `supersedes`; nothing is written on the older
record.

Sources, Requirements, and Rules may also carry `declared_by`. It names the
integration allowed to reconcile that record; it does not grant ownership of
the scope or of unrelated graph state. The typed SDK refuses implicit takeover
of an unowned record and all takeover of a foreign-owned record. An exact
`adopt_unowned` target can assign an unowned record to one declaration. The
target must name one declaration with the same explicit Stable ID. The fields
supplied by that declaration and its relevant relationships must already match.
Richer canonical metadata outside the typed surface remains unchanged and does
not block adoption. Adoption changes only `declared_by` and
`declaration_address`; it never rewrites a relation field.

Typed-owned Sources, Requirements, and Rules may carry `retired: true`.
Omitting the field means active. A complete typed declaration document retires
owned records from the same spec when they disappear, but keeps their canonical
IDs, addresses, and relation fields. Reintroducing the declaration clears the
field and reuses the same record. Active graph, gap, health, implementation, and
verification checks exclude retired declarations. Hard deletion is a separate,
unsupported operation.

Typed verification relationships live in
`scopes/<scope>/verifications/binding.jsonl`. Each row joins an owner-local
binding key and repository code location to a canonical Rule. Scanner markers
remain an equal source of verification relationships. No shard stores a
`verified` boolean: Unverified is derived only when neither source supplies a
live binding.

Typed implementation relationships live in
`scopes/<scope>/implementations/binding.jsonl`. Removing `implementedBy` from
an otherwise active typed Rule sets the owned binding's `retired` field instead
of deleting the row. Reintroducing it clears retirement on the same binding ID;
changing its exported target updates that row. Active coverage, wiki, stale,
health, and plan views exclude retired bindings. Export, import, checking, and
exact graph references preserve them as canonical history. Applying one spec
does not retire bindings attached to Rules declared by another spec, even when
both specs use the same declaration owner.

Typed verification relationships live in
`scopes/<scope>/verifications/binding.jsonl`. When a verification owner reports
one of its keys from a file against a different Rule, the binding that key
previously named gets its `retired` field set instead of being deleted.
Reporting it again clears retirement on the same binding ID. Active coverage,
wiki, stale, health, and plan views exclude retired bindings; export, import,
checking, and exact graph references preserve them. A run reconciles only the
owner, file, and key it reported, so another owner's binding and the same key
declared from another file stay active.

Requirement review records live in
`scopes/<scope>/requirements/review.jsonl`. When an applied reconciliation
restates a Requirement's `statement`, one row per affected Rule records the
Requirement, the field, both statements, and when the change landed. A review
is identified by that exact restatement, so re-applying the same change never
reopens a cleared review. A verification run for the Rule recorded after the
change sets `cleared_at` and `cleared_by_run` while keeping the reason; nothing
is deleted. Plan reads these rows to report review-required evidence, and
previews the reviews an unapplied diff would raise without writing them.

A Rule with no active implementation binding is a valid unimplemented Rule.
A rule's source document and section are citations and do not count as
implementation.

Callback-backed SDK runs are distinct volatile evidence. They are stored in
`.provenance/cache/scopes/<scope>/verification-runs.jsonl`, linked to a
canonical Verification binding and Rule by ID, and never enter canonical
shards. The durable binding does enter canonical exports; run outcomes do not.

Modern proposal definitions are immutable `proposed` rows. Assertions live in
`ideation/assertions.jsonl`; dispositions use `ideation/dispositions.jsonl`. Readers accept
the previously shipped `ideation/promotion_decisions.jsonl` path only for the exact frozen
historical audit. Import also accepts the old `promotion_decisions` export field instead of,
but never alongside, `dispositions`; all other top-level fields are closed. Effective state is
derived in the order
`proposed`, `asserted`, then disposition. `ideation/landings.jsonl` stores one validated
swarm batch per line so a run is published atomically. Import validates a staged complete
state directory and renames it into place only after `check` succeeds.

Pre-lifecycle terminal proposal rows are not rewritten. The compiled, versioned shipped-v1
fingerprint policy freezes both the exact shipped terminal definitions and their historical
disposition audit; any added, omitted, or changed frozen row fails validation. Embedded terminal
state remains authoritative even if lifecycle rows are present. The audit fingerprint includes
only dispositions targeting that frozen terminal set, so allowlisted modern lifecycle records can
coexist in the same scope. New terminal definitions are never accepted as modern ingress.

A modern disposition may omit or carry one closed `external_action` object with required
`system`, `scope`, `kind`, and `key` strings. Its identity is that exact four-part tuple and it
is preserved as part of the immutable audit row. Omission keeps shipped-v1 serialization and
fingerprints unchanged. A `canonical_artifact` resolves as the disposition scope plus exact
artifact type and ID; missing, cross-scope, and cross-kind collisions fail closed before direct
write or import publication and during repository checking and cache materialization.

Repository access first takes `.provenance/cache/locks/repository.publication.lock`. Writers then
take a scope lifecycle lock when applicable and finally a shard lock; this repository, lifecycle,
shard order is mandatory. Multi-shard writers hold the publication lock for their complete
operation, and aggregate readers hold it for their complete view or copy one locked snapshot.
Import holds the publication lock across recovery, snapshot, staged validation, and publication,
so cooperative readers and writers cannot observe or modify the brief directory-rename gap. Lock
files are derived cache artifacts, not state, and must not be committed.

Import publication uses a durable `.provenance/cache/import-publication.json` marker and unique
staging/backup directory. Repository access recovers an interrupted publication before reading:
if live state is absent, the backup is restored; if live state exists, pending backup cleanup is
finished. Files and containing directories are synced where the platform supports directory
`fsync`. Portable filesystems do not provide an atomic directory exchange, so the guarantee is
not overstated as crash-atomic: cooperating access never sees missing live state, interruption is
recoverable on next access, and any import command that returns failure leaves the old live state.

A graph reference canonicalizes a selected scope into a JSON object with fixed graph
families and records sorted by stable ID. JSON object keys are lexicographically ordered
before SHA-256 hashing. The projection contains the selected manifest scope and its
sources, domains, requirements, boundaries, topics, questions, resolutions, and rules;
relations travel as the fields on those records. Threads, messages, contributions,
synthesis packets, proposals, assertions, dispositions, cache data, and wiki output are
excluded.
