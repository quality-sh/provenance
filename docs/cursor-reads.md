# Cursor read contract

Resolution `res_review_reads_use_revision_bound_cursors` defines the approved
read behavior. This document specifies its operation interface.

## Contract

`search` and `read-document` use short projection transactions. Each continuation
checks the complete revision: instance ID, serial, digest, and read derivation.
A cursor binds the operation, canonical repository target, scope, normalized
query, filters, order version, and page limit. The host checks authorization
before each operation. A cursor grants no authority.

A caller sends the same request with the returned `cursor`. A null
`next_cursor` ends the sequence. Clients must not decode or modify cursors.
A refused continuation never returns a replacement first page. A caller must
discard the incomplete sequence before it starts again without a cursor.

## Document membership

The root must be a present, active Requirement. Members are that Requirement,
its active refinement descendants, the Resolutions that name those Requirements,
and the active Rules that name those Requirements or their Resolutions. Topics,
Questions, and Boundaries anchored to a member Requirement are also members.
Here, active means not retired. Lifecycle status does not establish retirement
or review approval.

Outgoing links from active members supply reference identities. References do
not cause membership expansion. Ancestors remain references. Other Requirement
branches remain cross-links. A retired Requirement or Rule does not expand the
active member set. A retired Source can retain its identity as a reference,
but is not an active citation. Missing references have no record entry.

Threads belong to their canonical member parent. Every Thread remains distinct,
including resolved and archived Threads. Messages belong to those Threads and
retain their logical counters. References do not import discussions.

The response identifies each entry as a member, reference, Thread, or Message.
Consumers can display loaded records before completion. Absence from an
incomplete sequence means unloaded, not empty or missing. Only a terminal
sequence at one revision establishes complete membership and discussions.
A failed catch-up does not establish a current document.

Visual depth does not change membership. The browser keeps the Requirement
root when it selects or expands a nested Rule or other record.

## Wire identities and compatibility

Operation protocol: **8**. Read derivation: **2**. State schema remains **2**.
HTTP uses `POST /v8/operations/search` and `POST /v8/operations/read-document`.
MCP uses the `search` and `read-document` tools with
`{"protocol_version":8,"call":<the HTTP body>}`. Both return the same stamped
query object. MCP also supplies its normal text copy. The MCP tool result is
therefore larger than the query object; its maximum is 3,342,592 bytes, excluding
JSON-RPC framing and the caller-supplied request ID. Native Rust uses `operations::queries::{search, read_document}` with
`SearchQuery` or `ReadDocumentQuery` and returns `Stamped<SearchResult>` or
`Stamped<ReadDocumentResult>`. `QueryResponse::new` supplies the wire envelope.

The TypeScript package entry is `@quality-sh/provenance/client`. Import
`HttpClient` and the generated `components` type from that entry. The exact
schema identities are:

| Purpose | `components['schemas']` key |
| --- | --- |
| Document call | `ReadDocumentRequestInput` |
| Document request fields | `ReadDocumentRequestInputReadDocumentQuery` |
| Document response | `ReadDocumentSuccessOutput` |
| Document entry union | `ReadDocumentSuccessOutputDocumentEntry` |
| Document failure envelope | `ReadDocumentFailureOutput` |
| Search call | `SearchRequestInput` |
| Search response | `SearchSuccessOutput` |
| Search failure envelope | `SearchFailureOutput` |

`HttpClient.readDocument(call)` and `HttpClient.search(call)` return those
success types. Rust generated HTTP types use the same PascalCase identities
under `provenance_http_client::types`; methods are `read_document` and `search`.
Do not copy these interfaces into the web repository. Derive record types from
the generated entry union. Type identities for nested records are operation
specific, even when they describe the same canonical record.

This is a source contract, not a claim about an available package release.
Published SDK 0.2.2 does not supply this contract. The package split is deferred.
Version 7 dispatch is refused. The unpublished full-scope document result is
replaced; no compatibility endpoint publishes that result. Other complete-list
operations do not become pages.

## Budgets and order

| Limit | Value and result |
| --- | --- |
| Returned entries or search matches | Default 50, maximum 200; zero and values above 200 are invalid input |
| Search candidates decoded | At most 512 per request; an empty page can have a continuation |
| Canonical row stored bytes | 65,536 bytes, checked by SQL before payload decoding |
| Encoded record or document entry | 65,536 UTF-8 JSON bytes; larger records produce `page_record_too_large` |
| Encoded entries within a page | 1,048,576 bytes; the next unreturned entry remains behind the cursor |
| Entire successful query envelope | 1,114,112 bytes; the engine counts serialization before returning; 256 bytes are reserved for external diagnostic framing |
| Candidate key ID | 1,024 bytes; larger keys produce `page_record_too_large` |
| Cursor | Maximum 8,192 encoded bytes |
| Document selection sets | At most 4,096 keys each for the refinement branch, producing Resolutions, members, references, and ancestors; overflow produces `page_budget_exceeded` |
| SQLite work | Progress checks every 1,000 VM instructions; more than 10,000 callbacks interrupts the page with `page_budget_exceeded` |

Search order is Source, Requirement, Resolution, Rule, Topic, Question, Domain,
Boundary, then canonical ID within each kind. An omitted or empty kind filter
selects the first six kinds. Text is trimmed and lowercased. Kind filters are
sorted and deduplicated. Matching remains a substring in an individual existing
searchable field. Message bodies are not searched. `has_more` means that the
query has unread work; it does not promise a match on the next page.

The first entry is the selected Requirement root. The remaining members come
next, then references, Threads, and Messages. Within members and references,
entries use kind rank and canonical ID. Threads and Messages use their logical
creation counter and canonical ID. A Message retains its Thread ID even if that Thread was on an
earlier page. Entry identity is role, record kind, and canonical ID. A record
that is a member does not also appear as a reference in the same document.

SQL selects root-related keys and bounded payloads. It does not fetch whole
scope record tables and then slice vectors. SQL joins and sorts are also subject
to the VM instruction budget. Each request can refuse if its root-related query
needs too much work. The limits do not promise that every arbitrarily large
branch can finish. They do not bound the separate freshness step: `catch_up`
can validate and materialize saved state before the short page transaction.
The byte limits bound returned data and decoded rows, not SQLite's total page
cache or operating-system memory.

## Cursor lifecycle and refusal

Tokens use URL-safe base64 payloads authenticated with HMAC-SHA-256. They contain
an identity hash, the full revision, and the last consumed key. They contain no
credential or repository path. The identity includes the canonical resolved
repository path, scope, operation, normalized selector, page limit, and order
version. Two host aliases for the same canonical repository resolve to the same
target identity, but each request still needs its own host grant.

The 32-byte signing key lives in the ignored repository cache as
`read-cursor.key`. Atomic exclusive creation avoids concurrent first-read races;
a temporary file supplies owner-only permissions on Unix. The first page read
needs permission to create this cache key. Later reads need read access to it. Native invocations
and local hosts sharing that cache can validate the same cursor. Cache loss,
key replacement, or repository movement can invalidate a cursor. No time-based
expiry or retained snapshot is promised. Cursors are replayable while their
identity and revision still match. They never advance a server-side offset.

| HTTP status | `error.kind` | Client action |
| --- | --- | --- |
| 409 | `cursor_invalid` | Discard the sequence; restart without the cursor after checking the selector |
| 409 | `cursor_revision_changed` | Discard the sequence; start a new first page |
| 409 | `page_budget_exceeded` | Stop this read and report the limit; do not label a partial sequence complete |
| 409 | `page_record_too_large` | Report the record limit; do not truncate its content |
| 409 | `document_root_missing` | Report a missing Requirement root |
| 409 | `document_root_retired` | Report the retired root separately from a missing root |
| 409 | `document_catch_up_failed` | Clear current-document status and report refresh failure |

Existing authentication, target, scope, protocol, and freshness refusals remain
in force. All failure variants occur before any successful page is returned.
A restart can happen repeatedly while another process changes the projection.

## Example calls

The first document request is:

```json
{
  "context": {"repository":"A","scope":"default","freshness":"catch_up"},
  "request": {"id":"req_review","limit":1}
}
```

A response has this form. The cursor and revision values below are illustrative;
clients must retain the actual returned values.

```json
{
  "protocol_version":8,
  "operation":"read-document",
  "root_id":"req_review",
  "limit":1,
  "has_more":true,
  "next_cursor":"<opaque cursor from this response>",
  "entries":[
    {"kind":"member","node":{"node_type":"requirement","schema_version":2,"scope_id":"default","id":"req_review","statement":"The review shows saved records.","status":"active"}}
  ],
  "stamp":{"instance_id":"<instance>","serial":42,"digest":"sha256:<digest>","derivation":2,"policy":"catch_up","attested":["boundaries","messages","questions","relations","requirements","resolutions","rules","sources","threads","topics"],"live":[]}
}
```

For continuation, retain `id` and `limit` and set `cursor` to the actual
`next_cursor`. `annotate_only` reads the stored projection at the cursor revision
without another catch-up. `catch_up` can include later saved changes, which then
cause a revision restart. Neither policy retains a historical snapshot.

```json
{
  "context":{"repository":"A","scope":"default","freshness":"annotate_only"},
  "request":{"id":"req_review","limit":1,"cursor":"<actual next_cursor>"}
}
```

Shared search uses the same continuation field:

```json
{
  "context":{"repository":"A","scope":"default","freshness":"catch_up"},
  "request":{"text":"review","node_types":["requirement","rule"],"include_retired":false,"limit":50}
}
```

Its response has `nodes`, `limit`, `has_more`, `next_cursor`, `operation: "search"`,
and the same stamp structure. A changed revision produces:

```json
{"protocol_version":8,"operation":"search","error":{"kind":"cursor_revision_changed"}}
```

## Client integration

Start a document sequence with `catch_up`. Save its instance, serial, digest,
and derivation. Append only successful pages from that sequence. The table
attestations can differ between pages because different pages read different
record families; they are not revision identity. Keep the supplied member and
reference roles. Do not expand membership from a reference or a retired record.

Keep incomplete membership, references, and discussions distinct from empty
lists. The terminal cursor establishes completeness for the sequence, not for
another root or a newer working copy. Keep the selected Requirement root when
expanding a nested Rule. A Requirement cross-link starts a separate document
read; another record kind can use its loaded canonical identity or `get`.
The existing `get` is a separate stamped read and must not be merged into an
incomplete document without checking revision identity.

The host document loader captures the selected root once per refresh. Its
callback accepts an optional cursor and returns a generated document page.
The dependent renderer must call that callback for continuation; the previous
whole-scope adapter cannot be used.

Use the shared generated `search` method to discover unloaded records. Retain
its cursor with the search selector and discard it when the selector changes.
Search results are navigation results, not an addition to document membership.
After a restart or failed catch-up, discard the incomplete accumulator. Preserve
the host's refresh generation check, credential handling, and read-only controls.

Production publication still needs the SDK release, a genuine web dependency
and lockfile update to that available release, a validated renderer archive and
exact checksum pin, then host composition. `provenance-web` owns Storybook
component tests and its one small browser smoke test of built assets. This
repository retains Rust host, authorization, asset, and API tests, and non-UI
session tests.
Local generated-client tests establish none of those publication facts.
