# Review reads, cursors, and search

Ben requested cursor pagination and search for the review page. After the research summary, he authorized the graph changes and implementation in the same Codex conversation on 2026-09-10. This document records the product and architecture contract. Beads records delivery order.

The supporting [research report](2026-09-10-cursor-search-research.md) cites the inspected repository commits. Its alternatives and open questions are research, not additional approved requirements.

## req_query_pages_preserve_revision

A page uses the revision established by the first page. A continuation cursor identifies the query, its filters and order, and its revision. Each request checks authorization. A changed revision produces an explicit restart response. Short read transactions avoid retaining a transaction between browser requests.

The shared engine bounds each page and its read work. The browser must not need all records in a scope before it can display a document. Exact page sizes, cursor encoding, and response budgets are implementation choices that require documented tests. Uninterrupted reads across concurrent writes are not promised.

This replaces the no-continuation position in `res_query_answers_stop_at_the_limit`. Existing page limits, per-list cut reporting, and file-scan limits remain unless the corresponding operation contract changes. An existing complete-list operation must not silently become a first-page operation.

## req_review_search_finds_unloaded_records

The review page uses the shared search operation to find canonical records that have not loaded in the browser. It exposes continuation and navigation to results. The initial behavior retains existing substring matching and record-kind filtering. New relevance ranking and discussion-body search are not required by this decision.

## req_review_document_has_root_relative_membership

Document membership follows the selected Requirement, its refinement descendants, and their production records. Upstream ancestors remain references or breadcrumbs. A cross-reference to another Requirement branch does not import that branch. Shared identities remain stable.

Retired records do not add members or citations to an active document. Historical identity remains distinct from a missing record. Full history controls remain separate.

A depth-limited view must still permit selection or expansion of a nested Rule or other record without treating it as a Requirement root. Unloaded lists remain distinct from empty lists. A failed catch-up or revision restart cannot be presented as a complete current document.

The graph records these obligations. It does not prescribe a release date, assign workers, or record task completion.
