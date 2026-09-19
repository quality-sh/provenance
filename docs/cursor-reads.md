# Cursor read contract

Resolution `res_review_reads_use_revision_bound_cursors` defines the approved
read behavior.

## Contract

Search and Requirement document reads use short projection transactions. Each
continuation checks the instance ID, serial, digest, and read derivation. A
cursor binds the operation, repository connection, scope connection, normalized
query, filters, order version, and page limit. The host checks authorization
before each request. A cursor grants no authority.

A caller sends the same GET parameters with the returned `cursor`. A null
`meta.next_cursor` ends the sequence. Clients must not decode or modify a
cursor. A refused continuation does not return a replacement first page. The
caller must discard the incomplete sequence before it starts again without a
cursor.

Search uses a GET query on a graph collection. For example:

```text
GET /requirements?query=search&text=review&limit=50
```

The assembled Requirement document uses:

```text
GET /requirements/req_review/document?limit=50
```

A successful list has this form:

```json
{
  "data": {"items": []},
  "meta": {
    "stamp": {"instance_id":"<instance>","serial":42,"digest":"sha256:<digest>","derivation":3,"policy":"catch_up","attested":[],"live":[]},
    "limit": 50,
    "has_more": true,
    "next_cursor": "<opaque cursor>"
  }
}
```

A changed revision returns the operation's typed 409 failure in the common
failure envelope:

```json
{"error":{"kind":"cursor_revision_changed"},"meta":{}}
```

`GET /metadata` supplies the complete compatibility tuple. There is no version
in the query URL, no per-call protocol field, and no response operation echo.
HTTP and MCP use the same envelope.

## Document membership

The root must be a present, active Requirement. Members are that Requirement,
its active refinement descendants, the Resolutions that name those
Requirements, and the active Rules that name those Requirements or their
Resolutions. Topics, Questions, and Boundaries anchored to a member Requirement
are also members. Here, active means not retired. Lifecycle status does not
establish retirement or review approval.

Outgoing links from active members supply reference identities. References do
not expand membership. Ancestors and other Requirement branches remain
cross-links. A retired Requirement or Rule does not expand the member set. A
retired Source can retain its identity as a reference, but it is not an active
citation. Missing references have no record entry.

Discussions belong to their canonical member parent. Every Discussion remains
distinct. Messages belong to their Discussion and retain their logical
counters. Legacy unassigned messages remain under their Discussion Container.
References do not import discussions.

Consumers can display loaded records before completion. Absence from an
incomplete sequence means unloaded. Only a terminal sequence at one revision
establishes complete membership and discussions. A failed catch-up does not
establish a current document.

## Client integration

Start a sequence with the configured catch-up policy. Save its instance, serial,
digest, and derivation. Append only successful pages from that sequence. Table
attestations can differ between pages because pages can read different record
families; they are not revision identity.

On `cursor_revision_changed`, discard all pages and start without a cursor. On
`cursor_operation_mismatch`, treat the cursor as a caller defect. On
`cursor_invalid`, do not interpret the token. On `cursor_read_derivation_mismatch`,
upgrade the client and restart. On stale or catch-up failures, show the failure
and do not present the partial sequence as complete.
