**Recommendation.** Use the existing `search` operation for discovery, add real continuation cursors, and make document reads select data related to the requested root. Start with **revision-bound pagination**: each page uses a short SQLite transaction and must match the revision established by the first page. If the projection changes, return a typed restart response. Do not retain transactions between browser requests initially.

PR227 solves truncation and coherence within one response, but its whole-scope preload is an implementation choice, not a demonstrated requirement of the review document. PR227 and Web PR11 should remain drafts while Ben reviews the inclusion and continuation contract.

This recommendation has a material cost: a busy repository can repeatedly invalidate pagination. If uninterrupted reading during concurrent writes is required, retained snapshots become justified. Ordinary keyset pagination alone does not provide that guarantee.

**Research scope and pins.** Both worktrees remained clean and at the supplied commits. No files, graph records, issues, commits, branches, releases, or pull requests were changed. No builds or additional agents ran. I read the repository instructions and the codebase-design and domain-modeling skills as research aids.

| Repository | Inspected draft head | `origin/main` merge base and verified current merged baseline |
|---|---|---|
| Provenance | `c836e1b94325b0167e237e21d827ab2487aadb25` | `ff515c78a398030bd694e3c9ae3707dbc5aaa590`, PR226 |
| Provenance Web | `96e4d5ae8f994a8b2758d06c9100d063a403c731` | `93678e4a3a3308b01300c57cc613788af4d54def`, PR10 |

GitHub confirmed that [PR227](https://github.com/quality-sh/provenance/pull/227) and [Web PR11](https://github.com/quality-sh/provenance-web/pull/11) are open drafts, unmerged, at those heads. Provenance contains four commits beyond its baseline; Web contains one.

The machine reports `Mint-Desktop`, Linux Mint 22. The available tools do not independently expose the selected model or reasoning setting, so I cannot attest the requested Astra/high selection. Beads read commands failed because the configured Dolt server lacks the `provenance` database. I did not repair it; conclusions about open work therefore cover GitHub and repository history, not a complete live Beads inventory.

**What is already merged.**

The merged search is useful and should be extended rather than duplicated:

- It matches a trimmed, case-insensitive substring against each record’s searchable fields, including its canonical ID.
- It accepts `node_types`, `include_retired`, and `limit`.
- Its default limit is 50; the maximum is 200.
- Results use node-kind rank, then canonical ID. They are not ranked by relevance.
- An omitted kind filter selects Source, Requirement, Resolution, Rule, Topic, and Question. Domain and Boundary require explicit selection.
- Blank text is invalid. There are no lifecycle, domain, owner, or document-membership filters in `SearchQuery`.
- Threads and Messages are not searchable graph-node kinds.

These facts come from the merged [search request](https://github.com/quality-sh/provenance/blob/ff515c78a398030bd694e3c9ae3707dbc5aaa590/crates/provenance-core/src/protocol/query.rs#L54), [search handler](https://github.com/quality-sh/provenance/blob/ff515c78a398030bd694e3c9ae3707dbc5aaa590/crates/provenance-store/src/operations/queries/records.rs#L25), and [searchable fields](https://github.com/quality-sh/provenance/blob/ff515c78a398030bd694e3c9ae3707dbc5aaa590/crates/provenance-core/src/protocol/node.rs#L74).

For example, this is a current, valid HTTP request:

```http
POST /v7/operations/search
Authorization: Bearer <local-host-token>
Content-Type: application/json

{
  "context": {
    "repository": "A",
    "scope": "default",
    "freshness": "catch_up"
  },
  "request": {
    "text": "review page",
    "node_types": ["requirement"],
    "include_retired": false,
    "limit": 50
  }
}
```

The result includes `nodes`, `limit`, `has_more`, operation identity, and a freshness stamp. **`has_more: true` means the result was cut; it does not provide a way to obtain the remainder.** Repeating the request obtains the first portion again. Search, neighbors, and trace request types contain no cursor. [Merged response contract](https://github.com/quality-sh/provenance/blob/ff515c78a398030bd694e3c9ae3707dbc5aaa590/crates/provenance-core/src/protocol/response.rs#L53)

The existing implementation also has a resource gap: search stops visiting later kinds once it has enough results, but each visited table’s SQL search uses `fetch_all`, without a SQL row limit. Thus a 50-result response does not establish a 50-record memory bound. The relation reader similarly fetches all edges touching a frontier before later filtering. Cursor work should address these reads, not merely slice their existing vectors. [Table reads](https://github.com/quality-sh/provenance/blob/ff515c78a398030bd694e3c9ae3707dbc5aaa590/crates/provenance-store/src/cache/read/records.rs), [relation-front reads](https://github.com/quality-sh/provenance/blob/ff515c78a398030bd694e3c9ae3707dbc5aaa590/crates/provenance-store/src/cache/read/front.rs#L64)

There are two other search implementations worth distinguishing:

| Existing search | Useful behavior | Limit for this task |
|---|---|---|
| Shared engine `search` | Canonical records, scope, retirement filter, stamps, native/HTTP/MCP access | No continuation or relevance ranking |
| Static Wiki search | All-word matching, title weighting, exact-title boost, query in navigation history | Searches preloaded DOM entries from a generated site |
| Web relationship picker | Subsequence fuzzy matching and ranking | Searches supplied local candidates; does not discover unloaded records |

The Wiki interaction and ranking ideas can inform the browser. Neither local matcher should be presented as a complete search over records that have not loaded. [Wiki search](https://github.com/quality-sh/provenance/blob/ff515c78a398030bd694e3c9ae3707dbc5aaa590/crates/provenance-cli/src/wiki/theme/search.js), [Web fuzzy matcher](https://github.com/quality-sh/provenance-web/blob/93678e4a3a3308b01300c57cc613788af4d54def/src/review/fuzzy.ts)

**Pagination did exist in earlier work.**

Your recollection is supported by code history.

1. Commit `d47af3853cc5b673ac9ec4733a8d0064468617f0`, in [PR169](https://github.com/quality-sh/provenance/pull/169), implemented offset cursors for search, neighbors, and impact; a trace resume token; and evidence continuation. PR169 was closed without merging.
2. Its search cursor was literally `v1:<offset>`. Search recomputed the matching vector and discarded the prefix. The trace token carried depth, rank, ID, and selected request parameters. These implementations did **not** bind continuation to a retained database snapshot or the current projection identity. They are useful historical examples, not a ready stable-cursor implementation. [Historical cursor codec](https://github.com/quality-sh/provenance/blob/d47af3853cc5b673ac9ec4733a8d0064468617f0/crates/provenance-core/src/protocol/stamp.rs#L77), [historical search](https://github.com/quality-sh/provenance/blob/d47af3853cc5b673ac9ec4733a8d0064468617f0/crates/provenance-store/src/operations/queries/served_graph.rs#L68), [trace token](https://github.com/quality-sh/provenance/blob/d47af3853cc5b673ac9ec4733a8d0064468617f0/crates/provenance-store/src/operations/queries/trace_token.rs)
3. A later plan proposed keyset cursors. Revision 2 explicitly removed pagination after the September 5 decisions. The merged graph records `res_query_answers_stop_at_the_limit`, whose rationale says that no caller had shown the need. [Plan revision](https://github.com/quality-sh/provenance/commit/7d82d4c87b9bcd63b4aa6f7ee23d06bcd8385a62), [merged Resolution](https://github.com/quality-sh/provenance/blob/ff515c78a398030bd694e3c9ae3707dbc5aaa590/.provenance/state/scopes/default/resolutions/res.jsonl#L51)

That explains the current implementation. It does not settle the present decision: this browser supplies a concrete need, and your current instruction asks to reconsider the earlier constraint.

I found no active pagination implementation in the current native query contract, HTTP/MCP adapters, or SDK sources, and no open pagination PR in the inspected GitHub inventory. [PR210](https://github.com/quality-sh/provenance/pull/210) adds a graph index for Wiki/gap reads; it is relevant to indexed joins, but does not supply a public cursor contract. MCP’s `PaginatedRequestParams` appears in tool discovery, where it is ignored; that is not graph-result pagination. [MCP listing](https://github.com/quality-sh/provenance/blob/ff515c78a398030bd694e3c9ae3707dbc5aaa590/crates/provenance-transport/src/mcp.rs#L33)

**What PR227 and PR11 actually require today.**

PR227 validates the supplied ID, copies it into `root_id`, then loads all Requirements, Resolutions, Rules, Sources, Topics, Questions, Threads, and Messages in the selected scope. The ID does not constrain those selects. The server does not first establish that the root exists. [Draft document handler](https://github.com/quality-sh/provenance/blob/c836e1b94325b0167e237e21d827ab2487aadb25/crates/provenance-store/src/operations/queries/document.rs#L18)

Its coherence benefit is real: all eight collections use one `ReadSnapshot`. The tests cover a publication between snapshot establishment and document reads, and separately cover 206 Requirements and retired identity. These are inspected test cases, not tests rerun in this research. [Draft document tests](https://github.com/quality-sh/provenance/blob/c836e1b94325b0167e237e21d827ab2487aadb25/crates/provenance-store/src/operations/queries/tests/document.rs)

The generated TypeScript and Rust HTTP clients refuse bodies above 16 MiB. However:

- The server already constructed the record vectors and JSON result.
- The TypeScript client accumulates chunks, combines them, decodes text, and parses JSON.
- The 16 MiB limit bounds accepted wire data, not total server or browser memory.
- The TypeScript size refusal is classified as `MalformedResponseError`, not a distinct document-too-large response.
- Native callers and MCP do not acquire this HTTP-client limit automatically.

The inspected HTTP adapter bounds **request** bodies; its success path serializes the returned value without a document response budget. [Client runtime](https://github.com/quality-sh/provenance/blob/c836e1b94325b0167e237e21d827ab2487aadb25/tools/operation-codegen/templates/http-runtime.ts#L40), [HTTP response path](https://github.com/quality-sh/provenance/blob/ff515c78a398030bd694e3c9ae3707dbc5aaa590/crates/provenance-transport/src/http.rs#L65)

PR11 converts the whole response into synchronous record maps. It requires successful `catch_up`, all eight attestations, and no live constituents; it rejects missing or retired roots. Re-rooting then operates entirely on those maps. [Draft read adapter](https://github.com/quality-sh/provenance-web/blob/96e4d5ae8f994a8b2758d06c9100d063a403c731/src/review/httpStore.ts)

The host adds credential entry, an explicit Requirement-ID input, and refresh. It does not call search. Refresh removes the previous view, and a generation counter prevents an older response from replacing a newer one. [Host application](https://github.com/quality-sh/provenance/blob/c836e1b94325b0167e237e21d827ab2487aadb25/tools/review-host/main.ts), [refresh handling](https://github.com/quality-sh/provenance/blob/c836e1b94325b0167e237e21d827ab2487aadb25/tools/review-host/session.ts)

**Whole-scope preload is an assumption of the current adapter, not a necessary document definition.**

There is an important qualification: the current builder needs more than the obvious descendants.

Its `collectScope` function follows refinement ancestors, finds children, includes resolving Resolutions and related Rules, and can bring in other Requirements through shared Rule and Resolution references. It then scans the supplied maps for more related records until the set stops growing. Separately, it scans supplied Requirements, Resolutions, and Sources for reverse supersession, and uses outside records to resolve reference titles. [Merged builder](https://github.com/quality-sh/provenance-web/blob/93678e4a3a3308b01300c57cc613788af4d54def/src/review/buildDocument.ts#L138)

Concrete consequences:

- Opening child `req_B`, where `req_B.refines = req_A`, can pull in `req_A` and then other children of `req_A`.
- A Rule with `requirement_ids = [req_A, req_X]` can widen membership to `req_X`.
- An otherwise external record that supersedes an included record can affect whether that included record is displayed.
- A missing map entry can mean “not fetched,” but the builder currently treats it as absent or unresolved.
- The renderer’s depth-four “continue” link limits nesting on screen; it does not limit the data already loaded or built.

These are code-derived implications, not measured performance results. [Supersession and placement](https://github.com/quality-sh/provenance-web/blob/93678e4a3a3308b01300c57cc613788af4d54def/src/review/buildDocument.ts#L309), [display depth](https://github.com/quality-sh/provenance-web/blob/93678e4a3a3308b01300c57cc613788af4d54def/src/components/review/RecordCard.tsx#L39)

A server can supply the required relationship closure, reference records, and supersession facts without loading unrelated collections. The current indexed relation reads and typed record lookups provide useful foundations. But feeding arbitrary pages into the unchanged builder would be incorrect: warnings and placement could change simply because another page arrived.

The architectural seam should distinguish **document membership**, **reference resolution**, and **visual placement**. Rust can select canonical records and relationship facts; TypeScript can retain rendering and placement policy. This needs a completeness contract, not a second copy of all presentation logic in Rust.

**Three alternatives.**

| Alternative | Benefits | Costs and risks | Assessment |
|---|---|---|---|
| Whole-scope bundle, as in PR227 | One coherent response; existing adapter works; immediate navigation among preloaded Requirements | Unrelated transfer; eager memory use; all-or-nothing size failure; no progressive discovery | Useful prototype evidence, weak default contract |
| Root-scoped reads with revision-bound cursors | Short transactions; bounded pages; reuses stamps, freshness, search, and relation readers; no retained database session | Revision changes cause restarts; root inclusion and partial rendering need explicit semantics; graph work needs separate bounds | **Recommended starting point** |
| Root-scoped reads with retained snapshots | Pages remain available at one revision while new writes occur | Snapshot registry or copied data; expiry, quotas, cleanup, restart behavior; WAL/disk costs | Use if uninterrupted continuation is a product requirement |

Paging all eight scope collections is also possible, but it retains most of the unrelated transfer and eventual client accumulation. It is a transport improvement, not a root-scoped document design.

**Recommended cursor contract — proposed, not existing.**

Use one shared cursor mechanism across the operation catalog, with operation-specific ordering and selectors.

| Concern | Recommended semantics |
|---|---|
| Token | Opaque, versioned, authenticated token. Base64 encoding alone is not tamper protection. |
| Identity | Bind operation, resolved repository identity, scope, authorization context, normalized query, filters, ordering version, document root/part where applicable, and projection revision. |
| Revision | Compare `instance_id`, `serial`, `digest`, and read derivation. A serial alone is insufficient. |
| Ordering | Search: existing kind rank plus canonical ID. Discussion lists: logical creation counter plus ID. Relation pages need relation/direction tie-breakers when one target occurs through multiple relationships. |
| Position | Continue strictly after the last emitted key. Do not use an offset into a newly computed result. |
| Limits | Keep 200 as a maximum **page** size. Also enforce a server response-byte budget and bounded query work. |
| Completion | A normal terminal page has `next_cursor: null`. A short page caused by a byte limit can still have a next cursor. |
| Replay | The same valid request at the same revision yields the same page. Read retries do not advance a mutable server offset. |
| Invalidity | Malformed, tampered, expired, mismatched, or invalidated tokens produce explicit failures; never silently restart at page one. |
| Authorization | Authenticate and authorize every request. A cursor grants no access by itself. Do not put credentials in it. |

Normalize the query according to the actual search semantics: trim/lowercase text, sort and deduplicate kind filters, and preserve substring behavior. Do not introduce extra whitespace or Unicode normalization silently. Initially freeze page-size and byte-budget choices within a sequence; allowing changes later is possible, but adds contract cases.

An illustrative starting budget is 50 records and 256 KiB per requested page, under a fixed server ceiling. These are proposed values, not performance findings. Measure and select final values before release.

A record larger than the page budget requires a named refusal or an explicit separately bounded content read. Silently truncating a canonical record is unacceptable. Large embedded relationship arrays and Message bodies mean that a record-count cap alone cannot bound memory.

Token signing also has a cost. A local native CLI needs a verification key available across invocations; a process-only key would invalidate every cursor when that process exits. A restricted, noncanonical cache key is one option. Hosts can use issuer-scoped keys. Key rotation or cache loss must invalidate tokens cleanly. Cross-host token portability should not be promised by default.

**Consistency and freshness.**

The merged reader already does the hard part for **one call**: establish a SQLite transaction by reading the stored revision, read through attested handles, then consume the context and close its connection. It does not retain old revisions for subsequent calls. [Snapshot lifecycle](https://github.com/quality-sh/provenance/blob/ff515c78a398030bd694e3c9ae3707dbc5aaa590/crates/provenance-store/src/operations/reader/snapshot.rs#L31), [read completion](https://github.com/quality-sh/provenance/blob/ff515c78a398030bd694e3c9ae3707dbc5aaa590/crates/provenance-store/src/operations/reader/freshness.rs#L31)

For revision-bound pagination:

1. The initial request runs `catch_up` and establishes revision **R**.
2. Continuations and sibling document reads request **R**, using short stored-projection reads.
3. The server opens the transaction, reads its revision, and compares it with **R inside that transaction**.
4. If equal, it reads the page there. If different, it returns `revision_changed`.
5. Refresh starts a new sequence after catch-up. Pages from different sequences never merge.

This permits coherent continuation without retaining a snapshot **only while R remains the stored revision**. It is a conditional guarantee, not historical read access.

Ordinary keyset pagination cannot substitute for that check. Suppose page one ends at `req_100`. A concurrent edit can make an earlier record start matching, retire a later record, or change a relevance score. Continuing with `id > req_100` does not reproduce the original result set.

The current projection digest covers stored families across scopes, and catch-up has no journal. Consequently:

- Unrelated changes can invalidate a document cursor.
- Repeated `catch_up` on every page repeats freshness work and can invalidate the sequence itself.
- Using `annotate_only` between refreshes preserves the stored revision when nobody else updates it, but does not prove the canonical files remain unchanged.
- The present facilities cannot supply an incremental “changes since cursor” feed.
- A cache rebuild changes the instance identity; matching serial numbers after rebuild are not continuity.

The existing server guidance already suggests stored reads between refreshes and explicitly prohibits keeping a `ReadContext` across calls. Some later-stage wording in that document is stale; current code, rather than those pending-stage notes, establishes that `refuse_stale` exists. [Cache identity and catch-up](https://github.com/quality-sh/provenance/blob/ff515c78a398030bd694e3c9ae3707dbc5aaa590/docs/cache.md#L28), [server guidance](https://github.com/quality-sh/provenance/blob/ff515c78a398030bd694e3c9ae3707dbc5aaa590/docs/cache.md#L159)

Retained snapshots change this tradeoff. SQLite WAL readers can continue seeing their established snapshot during later commits, but long reads can prevent checkpoint completion and allow WAL growth. SQLite’s separate snapshot API has build and validity prerequisites; its availability must not be assumed from the repository’s `ReadSnapshot` name. [SQLite isolation](https://www.sqlite.org/isolation.html), [WAL checkpointing](https://www.sqlite.org/wal.html), [snapshot API](https://www.sqlite.org/c3ref/snapshot_get.html)

A retained design would need idle and absolute expiry, active-reader quotas, resource accounting, cleanup after disconnect/crash, and explicit expiry responses. Copying selected data into immutable storage avoids a long database transaction but consumes memory or disk and still needs eviction. A retained snapshot stays coherent; it does not stay current.

Existing commit-pinned graph references are not a drop-in substitute: they omit collaboration records and do not represent saved uncommitted working-copy state. [Pinned graph families](https://github.com/quality-sh/provenance/blob/ff515c78a398030bd694e3c9ae3707dbc5aaa590/crates/provenance-store/src/graph_reference/projection.rs#L11)

**Root inclusion and discussion coherence.**

My preferred inclusion rule is a Requirement’s structural document, with external relationships represented as links:

- Include the requested Requirement and structural descendants through refinement and production.
- Include associated Resolutions, Rules, Topics, Questions, and cited Sources.
- Preserve shared records once by canonical identity.
- Represent ancestors, other Requirements named by shared records, dependencies, contradiction targets, and other external references without automatically expanding their documents.
- Resolve supersession and retirement facts needed to display included records correctly.
- Load discussions for included records, and Messages for each selected Thread.
- Opening an external Requirement starts another root-scoped read and preserves the navigation trail.

**This narrows the current builder’s closure and needs Ben’s decision.** Preserving that closure is a valid alternative, but it can grow towards an entire connected region. Root scoping reduces unrelated data; it cannot guarantee a small document.

Use fixed selectors for document parts or expansions, rather than a general graph-query language. Page a direct group or discussion list independently. A general recursive traversal cursor needs a frontier and visited set, deterministic replay, or retained traversal state; a last-node ID alone is insufficient.

Placement must not depend on which page arrived first. In particular, the current “first included Requirement by ID” placement for shared records requires membership knowledge. Establish that fact before final placement, or mark placement pending. Do not attach a record provisionally and treat its movement after another page as a graph edit.

For coherence, graph records, relationship membership, Threads, and Messages must share R. Current `list-threads` and `list-messages` return complete native arrays through `StateStore`, without this shared stamped-read contract; combining them with paged graph reads would reintroduce mixed revisions. PR227’s snapshot-backed discussion access is useful work to retain and narrow. [Merged discussion-list adapter](https://github.com/quality-sh/provenance/blob/ff515c78a398030bd694e3c9ae3707dbc5aaa590/crates/provenance-store/src/operations/catalog/scoped_list.rs), [draft snapshot discussion reads](https://github.com/quality-sh/provenance/blob/c836e1b94325b0167e237e21d827ab2487aadb25/crates/provenance-store/src/cache/read/discussions.rs)

`stamp.attested` says which tables backed a response. It does **not** say their contents, a document, or a discussion are complete. Completeness needs separate metadata.

Retirement also remains separate from lifecycle. Active placement excludes retired Sources, Requirements, and Rules, while reference resolution must distinguish a retired record from an absent or unloaded record. Historical inspection can request retired content explicitly. [Accepted retirement semantics](https://github.com/quality-sh/provenance/blob/ff515c78a398030bd694e3c9ae3707dbc5aaa590/docs/adr/0004-typed-declarations-retire-in-place.md)

**Illustrative proposed requests and failures.**

The following shapes are design examples, not current callable interfaces. Version `8` illustrates a next protocol version. Record bodies are abbreviated.

A search continuation could retain the existing result fields:

```json
{
  "protocol_version": 8,
  "operation": "search",
  "stamp": {
    "instance_id": "projection-A",
    "serial": 42,
    "digest": "sha256:example",
    "derivation": 1,
    "policy": "catch_up",
    "attested": ["requirements"],
    "live": []
  },
  "limit": 50,
  "has_more": true,
  "next_cursor": "<opaque-authenticated-token>",
  "nodes": [
    {
      "node_type": "requirement",
      "id": "req_review_page_replaces_wiki"
    }
  ]
}
```

The continuation uses the same text and filters:

```json
{
  "context": {
    "repository": "A",
    "scope": "default",
    "freshness": "annotate_only"
  },
  "request": {
    "text": "review page",
    "node_types": ["requirement"],
    "include_retired": false,
    "limit": 50,
    "max_bytes": 262144,
    "cursor": "<opaque-authenticated-token>"
  }
}
```

For sibling document sections, an explicit revision condition permits shared identity without sharing a cursor position:

```json
{
  "context": {
    "repository": "A",
    "scope": "default",
    "freshness": "annotate_only"
  },
  "request": {
    "id": "req_review_page_replaces_wiki",
    "part": "threads",
    "anchor": {
      "node_type": "requirement",
      "id": "req_review_page_replaces_wiki"
    },
    "at_revision": {
      "instance_id": "projection-A",
      "serial": 42,
      "digest": "sha256:example",
      "derivation": 1
    },
    "limit": 50,
    "max_bytes": 262144
  }
}
```

A section result would identify its root, part, anchor, stamp, records, and continuation. `complete: true` would mean **that selected list** is exhausted, not that every descendant and discussion is loaded.

Suggested failure meanings:

| Condition | Response | Browser behavior |
|---|---|---|
| Token altered or query/filter changed | 400 `invalid_input`, field `request.cursor` | Discard the invalid sequence; do not append |
| Stored revision changed | 409 `revision_changed` | Keep prior content marked stale; offer refresh |
| Cursor expired/key changed | 409 `cursor_expired` | Start a new sequence |
| Catch-up failed | Preserve existing failure stamp/cause | Do not call the view current |
| Missing/retired viewed root | Explicit root-unavailable reason | Explain absence or retirement |
| One record exceeds server budget | Named `record_too_large` refusal | Explain the limit; do not truncate |
| Scope/access no longer granted | Existing authorization refusal | Stop reads and clear protected content as appropriate |

For example:

```json
{
  "protocol_version": 8,
  "operation": "read-document",
  "error": {
    "kind": "revision_changed",
    "expected_serial": 42,
    "current_serial": 43,
    "restart_required": true
  }
}
```

The serial fields above are diagnostic shorthand; the actual comparison must use the complete revision identity. New failure variants require schema and client support.

**Search and browser flow.**

The browser should begin with search, not require knowledge of a canonical ID.

1. Search Requirements through the existing operation. Show statement, ID, lifecycle, and an explicit retired indicator when historical results are requested.
2. Offer the existing kind filter for broader discovery. Non-Requirement hits need a record view or a choice of related Requirement documents; do not arbitrarily select one parent.
3. Keep query, filters, selected root, and navigation trail in browser history. Keep credentials and cursor state out of shareable URLs.
4. Show the root first, with unloaded groups clearly marked. Fetch visible or opened groups and discussions progressively.
5. Provide keyboard-accessible “Load more” controls and loading/error announcements. Infinite scrolling can supplement them.
6. Follow an external Requirement by fetching its document. Back navigation can reuse bounded cached content at the same revision.
7. Refresh into a separate generation. Adopt the new generation without combining it with old pages.

For the first cursor release, preserve existing search ordering and substring matching. Relevance ranking is a separate improvement to **the same search operation**. If added, rank over the complete candidate set with a versioned scoring rule and canonical tie-breakers. Sorting each received page by relevance produces no coherent global order. Changes to text, filters, ranking mode, or ranking version must start a new cursor sequence.

Likewise, lifecycle/domain filters must run before pagination on the server. Filtering only the first 200 records in the browser can hide matching records later in the result set. “Search this document” requires server-side membership semantics; until available, label a local filter “Search loaded records.”

Use independent state dimensions:

| Dimension | Examples |
|---|---|
| Loading/completeness | Unloaded, loading, partial, selected list complete, failed |
| Freshness | Caught up when opened, stored revision, newer revision detected, catch-up failed |
| Reference state | Unresolved, present, retired, missing, outside this document |
| Review state | Absent, pending, accepted/rejected for the reviewed version |

Examples of accurate UI text are “50 results loaded; more available,” “Messages not loaded,” and “Showing the revision opened earlier; newer data is available.” A collapsed or unloaded discussion must not say “No comments.”

A document is complete only when its selected inclusion closure, required reference checks, and included discussion lists are exhausted. Depth or work-budget stops must remain visible as incomplete. `has_more: false` from a depth-limited trace is not proof of a complete document.

Keep a bounded browser cache and bounded mounted content. Loading pages but retaining every object and repeatedly rebuilding the full tree merely moves the memory problem. Returning to an evicted section may require another read or a revision restart.

**Compatibility and release consequences.**

The shared operation catalog should own pagination behavior, validation, and failures. HTTP, MCP, native Rust/CLI entry points, generated Rust and TypeScript clients, and the TypeScript SDK wrapper should use that contract. The TypeScript SDK already delegates `search` to its generated client. [SDK wrapper](https://github.com/quality-sh/provenance/blob/ff515c78a398030bd694e3c9ae3707dbc5aaa590/packages/provenance/src/index.ts#L264), [generation contract](https://github.com/quality-sh/provenance/blob/ff515c78a398030bd694e3c9ae3707dbc5aaa590/tools/operation-codegen/README.md)

Important consequences:

- Current request types reject unknown fields. An older host cannot accept a new cursor field.
- HTTP dispatch and clients check protocol versions. New failures and partial-document semantics should be released under an explicitly compatible protocol strategy; a next-version contract is safer than assuming additive compatibility.
- Adding fields to public Rust request structs can break native struct-literal callers even when JSON fields are optional.
- A legacy complete-list operation must not silently start returning one page.
- MCP needs the same continuation and completeness data, with bounded responses suitable for its consumers.
- Page limits must apply in the shared handler, not only the browser or HTTP client.
- Cursor/order versions, operation protocol, read derivation, state schema, and package version are distinct. Pagination alone need not change canonical state schema or `graph-reference-v1`.
- Generated artifacts remain generated; no new language or query DSL is needed.

Published SDK `0.2.2` lacks `readDocument`, as the drafts document. The final contract therefore determines the SDK release, Web dependency and genuine lockfile update, renderer archive validation, and host asset pin. The current PR10 asset pin does not deliver PR11’s adapter. No publication should establish the bulk contract merely to unblock the dependent draft. [Draft packaging constraints](https://github.com/quality-sh/provenance/blob/c836e1b94325b0167e237e21d827ab2487aadb25/docs/review-host.md#L108)

**Choices still requiring Ben’s review.**

The strongest unresolved choices are:

- Whether a viewed child includes ancestors and their other branches, or keeps them as links.
- Whether shared Rules/Resolutions expand other Requirement documents automatically.
- Whether uninterrupted pagination during writes is required, or a visible restart is acceptable.
- Whether stale content remains visible during refresh/failure. PR227 currently removes it.
- Which filters and relevance behavior are needed beyond the merged substring search.
- Whether discussion bodies and Domain/Boundary views belong in this delivery.
- Final page/byte/work limits, oversized-record behavior, and cursor lifetime.
- Whether native and hosted cursors need portability across processes or hosts.

A global revision check is deliberately conservative. A later scope- or document-specific fingerprint could reduce unrelated invalidations, but would have to cover every membership edge, reference fact, supersession fact, and discussion that affects the result. It should not be introduced casually as an optimization.

**Impact on the drafts and `.3`.**

PR227 contains reusable work: shared registration, snapshot-backed discussion reads, working-copy freshness handling, coherence tests, authorization tests, and refresh-race handling. The proposed revision should replace its unconditional scope-wide selects and client-only size refusal with bounded selection and explicit continuation/completeness semantics.

PR11 contains reusable read-only controls, retirement handling, canonical identity preservation, and independent discussion rendering. Its synchronous complete-map assumption needs a load-state-aware adapter. The existing builder cannot safely consume arbitrary partial maps unchanged.

For `provenance-boc5.3`, search, coherent document reads, progressive navigation, and honest partial/stale states are a coherent read-only scope. `.4` action mapping, `.5` writes, and `.6` Wiki removal remain separate. Approval continues to accept the reviewed version; lifecycle remains independent; discussion counters remain logical counters.

The evidence supports revisiting the old no-cursor decision. It supports neither declaring whole-scope bulk approved nor claiming a cursor alone solves document loading. The recommended contract is **existing search plus bounded root-related reads, with explicit revision checks and explicit completeness**.