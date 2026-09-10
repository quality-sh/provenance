# Record updates

The operation catalog exposes edits to existing graph fields. The native CLI,
Rust operation calls, and generated HTTP clients use the same Store writers.
An update retains the record ID, schema version, scope, declaration address,
and creation origin. It does not write a new canonical record shape.

## Fields

| Operation | Fields that can change | Nullable fields that can be cleared |
| --- | --- | --- |
| `update-source` | `name`, `source_type`, `url`, `reference`, `commit_pin`, `effective_date`, `review_date`, `retired` | `url`, `reference`, `commit_pin`, `effective_date`, `review_date` |
| `update-resolution` | `title`, `position`, `rationale`, `status`, `context`, `enforcement`, `confidence`, `inputs`, `made_by`, `approved_by`, `approved_at`, `review_on` | `context`, `enforcement`, `confidence`, `made_by`, `approved_by`, `approved_at`, `review_on` |
| `update-requirement` | `statement`, `description`, `fog`, `status`, `domain_id`, `retired` | `description`, `fog`, `domain_id` |
| `update-rule` | `name`, `description`, `statement`, `status`, `severity`, `source_document`, `source_section`, `retired` | `name`, `description`, `source_document`, `source_section` |
| `update-domain` | `name`, `description`, `color` | `description`, `color` |
| `update-boundary` | `statement`, `source_ref` | `source_ref` |
| `update-topic` | `title`, `status`, `links` | None |
| `update-question` | `question`, `resolution_method`, `status`, `links`, `resolution_id`, `contradicts` | `resolution_id`, `contradicts` |

Each request contains `scope_id` and `id`. The request scope must equal the
selected context scope. The request cannot change either value on a record.
Unknown fields, unknown enum values, missing IDs, and invalid references are
refused. Updated required text must not be blank. A Domain name must remain
unique in its scope. Source commit pins and Resolution confidence and input
content use the existing validators.

An omitted field retains its value. A `null` field also supplies no value;
it does not clear the saved field. To clear a nullable field, name it in
`clear_fields`. The list is typed for each record kind. A required field is
not a valid list entry. A request that both supplies a value and clears the
same field is refused before publication. An empty array replaces a supplied
list with an empty list where that list is optional.

`inputs` replaces the Resolution input list. To add a later input, supply
the existing inputs with the new input appended. Input references retain
the native citation semantics; they are not graph relationships. Approval
fields describe the supplied record. They do not authenticate the approver
or define a new approval action.

For a Source, Requirement, or Rule owned by a typed declaration, the request
must supply the exact existing `declared_by` value. For a manual record,
omit it. This field is an ownership precondition, not an assignment. Updates
cannot adopt a record, change its owner, or move its declaration address.
`apply` retains its existing reconciliation, adoption, move, and retirement
behavior. A later declaration application can restate the fields it owns.

A Requirement statement change uses the statement write gate and raises the
existing Requirement reviews for its Rules. Rule statements use the same
write gate. Reviews retain their existing before/after values and clearing
behavior. Requirement and review publication uses more than one shard; a
failure after publication starts reports `uncertain_write`.

`retired` uses the existing Source, Requirement, and Rule retirement field.
Rule deprecation and archival use the existing `status` values. Neither
operation deletes records or their relationships. Other record kinds gain
no retirement state. Topic status changes clear claims when the topic closes.
Question status changes retain the native answer requirement and clear
claims when the question leaves its claimable state.

## Native CLI

All eight record command groups accept `update`. Changed fields are JSON or
an `@file` argument. For example:

```sh
provenance sources update --repo . --scope default --id source_policy \
  --fields-json '{"url":"https://example.test/policy","clear_fields":["reference","commit_pin"]}' \
  --format json

provenance resolutions update --repo . --scope default --id resolution_policy \
  --fields-json '{"status":"approved","approved_by":"reviewer","approved_at":1234}' \
  --format json
```

The JSON must not contain `id` or `scope_id`; use the command flags.
Questions also retain the existing update flags and add `--question`.
`--fields-json` cannot be combined with those individual question flags.
The CLI operates on local data and does not need `serve`.

## HTTP clients

Use the matching generated method, such as `updateSource` in TypeScript or
`update_source` in Rust. The request uses the existing scoped call envelope:

```json
{
  "context": {"repository": "selected", "scope": "default"},
  "request": {
    "scope_id": "default",
    "id": "source_policy",
    "url": "https://example.test/policy",
    "clear_fields": ["reference", "commit_pin"]
  }
}
```

The explicit clear list survives serialization in both generated clients.
Clients connect to an existing authorized host. They do not start a server,
manage a process, or retry mutations after a lost response.

## Existing shaping and relationship actions

The catalog also exposes `create-domain`, `create-boundary`, `create-topic`,
and `create-question`, with the existing native creation inputs. It exposes
`claim-topic`, `release-topic`, `close-topic`, `claim-question`,
`release-question`, and `answer-question`. Claims retain native eligibility
and claim clearing. Answering uses the existing answer writer.

Relationship operations retain the native target, cycle, and required-list
checks:

- Requirement: `set/clear-requirement-refines`,
  `add/clear-requirement-depends-on`, `add/clear-requirement-supersedes`,
  and `set/clear-requirement-spawned-by`.
- Rule: `add/clear-rule-requirement` and `add/clear-rule-resolution`.
- Resolution: `add/clear-resolution-requirement` and
  `add/clear-resolution-supersedes`.
- Source: `add/clear-source-supersedes`.
- Question: `set/clear-question-contradicts`.
- Source citations: the existing `add-source-reference` and
  `clear-source-reference`, which removes all citations of one Source from
  one Requirement.

Here `add/clear` denotes two operation names. Each new relationship request
contains `scope_id`, the owner `id`, and `target_id`. Clearing a single
reference omits `target_id`. Required lists keep their last entry. Several
relationship calls do not form one transaction.

The catalog exposes the existing `create-contribution`,
`upsert-contribution`, `create-synthesis-packet`, and
`upsert-synthesis-packet` writers. Upsert retains native full-record
replacement semantics and the assertion guards that protect history. The
native CLI retains `create --replace` for those drafts.

## Limits

These operations do not add stored edit-to-discussion links, a content
journal, Source document editing, complete document reads, review-button
mappings, or hard deletion of evidence. Question answers remain on the
existing answer path. Proposal, Assertion, and Disposition history retains
its existing lifecycle writers. Bulk ideation landing stays native-only.
Graph query pages retain their 200-record bound and have no cursor.
