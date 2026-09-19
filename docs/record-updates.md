# Record updates

The operation catalog exposes edits to the fields of existing graph records.
Native operation calls, the generated CLI dialect, and generated HTTP clients
use the same Store writers. An update keeps the record ID, schema version,
scope, declaration address, and creation origin. It does not write a new
canonical record shape.

The public write surface is one pattern:

```text
PATCH /{collection}/{id}
```

The collections are `sources`, `requirements`, `resolutions`, `rules`,
`domains`, `boundaries`, `topics`, and `questions`. The native operation names
(`update-source`, `update-resolution`, and the other `update-*` names) name
the same writers. The Requirement PATCH uses the guarded review writer.

## Fields

| Record | Fields that can change | Nullable fields that can be cleared |
| --- | --- | --- |
| `update-source` | `name`, `source_type`, `url`, `reference`, `commit_pin`, `effective_date`, `review_date` | `url`, `reference`, `commit_pin`, `effective_date`, `review_date` |
| `update-resolution` | `title`, `position`, `rationale`, `status`, `context`, `enforcement`, `confidence`, `inputs`, `made_by`, `approved_by`, `approved_at`, `review_on` | `context`, `enforcement`, `confidence`, `made_by`, `approved_by`, `approved_at`, `review_on` |
| `update-requirement` | `statement`, `description`, `fog`, `status`, `domain_id` | `description`, `fog`, `domain_id` |
| `update-rule` | `name`, `description`, `statement`, `status`, `severity`, `source_document`, `source_section`, `archived_in_commit` | `name`, `description`, `source_document`, `source_section` |
| `update-domain` | `name`, `description`, `color` | `description`, `color` |
| `update-boundary` | `statement`, `source_ref` | `source_ref` |
| `update-topic` | `title`, `status`, `links` | None |
| `update-question` | `question`, `resolution_method`, `status`, `links`, `resolution_id`, `contradicts` | `resolution_id`, `contradicts` |

The rows name the native operations. The PATCH route of the matching
collection carries the same editable set. The relationship fields of Source,
Resolution, Rule, and Requirement records are in their own section below.

## PATCH semantics

The path addresses the record. The connection carries the repository and the
scope. A request body is one object under `data`. The body must not repeat an
identity fact: `context`, `id`, and `scope_id` in the body are refused. A
success response puts the saved record in `data`. A failure returns
`{error, meta}`.

An omitted field keeps its value. A `null` value clears a clearable field in
the table. An array supplies the complete final value of that field. An empty
array empties an optional list.

The typed `clear_fields` list is the native clear form. A PATCH also accepts
it. The list is typed for each record kind, so a required field is not a valid
entry. A request that supplies a value and clears the same field is refused
before publication.

A `null` value for a field outside the clearable set currently supplies no
value: the request is not refused, and the field does not change. This gap is
separate pending work.

Unknown fields, unknown enum values, missing identity, and invalid references
are refused. Updated required text must not be blank. A Domain name must stay
unique in its scope. Source commit pins and Resolution confidence and input
content use the existing validators.

## Relationship deltas

Relationship edits belong to the PATCH of the owning record. No relationship
has its own route. These record fields hold relationship lists:

- Source: `supersedes`.
- Resolution: `requirement_ids`, `supersedes`.
- Rule: `requirement_ids`, `resolution_ids`.

Each field takes a complete array or a partial delta:

```json
{"supersedes": {"add": ["source_old"], "remove": []}}
```

The Requirement PATCH carries its relationships in one `relationships`
object. `refines` and `spawned_by` take a record ID, or `null` to clear.
`depends_on` and `supersedes` take an array or a delta. `cites` takes the
citation list or a delta with `add` citations and `remove` source IDs. This
example adds one dependency and clears the refine target:

```json
{"data": {"actor": "agent",
  "relationships": {"depends_on": {"add": ["req_b"]}, "refines": null}}}
```

The Store expands each edit against the saved record and validates the
complete final state. Text and relationships in one PATCH validate and
publish together. Required lists keep their last entry.

`inputs` replaces the Resolution input list. To add a later input, send the
existing inputs with the new input appended. Input references keep the native
citation semantics; they are not graph relationships. Approval fields describe
the supplied record. They do not authenticate the approver or define a new
approval action.

## Ownership and the review gate

For a Source, Requirement, or Rule owned by a typed declaration, the request
must supply the exact existing `declared_by` value. For a manual record, omit
it. This field is an ownership precondition, not an assignment. Updates cannot
adopt a record, change its owner, or move its declaration address.
`apply-authoring` keeps its reconciliation, adoption, move, and deletion
behavior. A later declaration application can restate the fields it owns.

The Requirement PATCH is the one guarded write path. The client sends the
current ETag in `If-Match` and a request identity in `Idempotency-Key`. The
member read returns the ETag. The response carries the record with its edit
state, decision state, and the new ETag. A statement change uses the statement
write gate and raises the existing Requirement reviews for its Rules. Rule
statements use the same write gate. Reviews keep their before/after values and
clearing behavior. Requirement and review publication uses more than one
shard. The Store resolves an interrupted publication. The wire returns
`write_failed` when it cannot return the saved result.

Rule deprecation and archival use the existing `status` values. An archived
Rule must have `archived_in_commit`; the other Rule statuses must not have it.
Updates do not delete records or their relationships. A Topic that closes
clears its claim. A Question status change keeps the native answer requirement
and clears claims when the question leaves its claimable state.

## Native CLI

Record commands use the resource address. The record ID comes from the
address, and `--scope` selects the scope. Scalar fields use flags. Arrays and
objects use one JSON object on standard input with `--stdin`:

```sh
printf '%s' '{"url":"https://example.test/policy","reference":null}' |
  provenance --repo . --scope default sources source_policy update

printf '%s' '{"status":"approved","approved_by":"reviewer","approved_at":1234}' |
  provenance --repo . --scope default resolutions resolution_policy update
```

Here `null` clears `reference`, as on the route. A body that carries `id`,
`scope_id`, or a header fact is refused.

A Requirement update sends the ETag of the last read. The CLI supplies the
`actor` default and makes an `Idempotency-Key` when the route needs one:

```sh
printf '%s' '{"description":"Reviewed wording."}' |
  provenance --repo . --scope default requirements req_policy update \
    --if-match "$etag"
```

## HTTP clients

Use the matching generated method, such as `updateSource` in TypeScript or
`update_source` in Rust. The method takes the record ID, the header controls
that the route declares, and the `data` object:

```json
PATCH /sources/source_policy
{"data": {"url": "https://example.test/policy", "reference": null}}
```

The `null` clears `reference`. Clients connect to an existing authorized host
with one bound repository and scope. They do not start a server, manage a
process, or retry a mutation after a lost response.

## Native operation requests

Native Rust calls use the typed request structs of the shared catalog. These
requests carry `scope_id` and `id`, and the request scope must equal the
selected scope. In the native DTO, a `null` field supplies no value; it does
not clear. A native request clears fields only through the typed
`clear_fields` list.

The relationship actions stay native: `set/clear-requirement-refines`,
`add/clear-requirement-depends-on`, `add/clear-requirement-supersedes`,
`set/clear-requirement-spawned-by`, `add/clear-rule-requirement`,
`add/clear-rule-resolution`, `add/clear-resolution-requirement`,
`add/clear-resolution-supersedes`, `add/clear-source-supersedes`,
`set/clear-question-contradicts`, and the citation actions
`add-source-reference` and `clear-source-reference`. Here `add/clear` names
two operations. A relationship request carries `scope_id`, the owner `id`,
and `target_id`; a singleton clear omits `target_id`. These actions have no
HTTP route. The public surface changes relationships through the owning
record PATCH.

## Shaping, claims, and drafts

`POST /{collection}` creates `domains`, `boundaries`, `topics`, and
`questions` with the existing native creation inputs. `claim`, `release`,
`close`, and `answer` are POST actions on the record path: `claim-topic`,
`release-topic`, `close-topic`, `claim-question`, `release-question`, and
`answer-question`. Claims keep native eligibility and claim clearing.
Answering uses the existing answer writer.

`POST /contributions` and `POST /synthesis-packets` create one draft.
`PATCH /contributions/{id}` and `PATCH /synthesis-packets/{id}` upsert one
draft. Upsert keeps the native full-record replacement semantics and the
assertion guards that protect history.

## Limits

These operations do not add stored edit-to-discussion links, Source document
editing, review-button mappings, or hard deletion of evidence. The assembled
Requirement document has its own read at `GET /requirements/{id}/document`.
Question answers stay on the answer action. Proposal, Assertion, and
Disposition history keeps its existing lifecycle writers. Bulk ideation
landing stays native-only.

Page limits keep the 200-record bound. Search and Requirement document reads
continue a sequence with a cursor; [cursor-reads.md](cursor-reads.md) defines
that contract.
