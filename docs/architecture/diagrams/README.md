# Architecture diagrams

These draw.io files show the code at commit
`e93799a40e7ca8c4fb00aed5e0882f05e91c9fe5` (main). The files are plain
uncompressed XML. Open them in draw.io or diagrams.net.

Each diagram uses one colour for each layer and has a legend. Grey arrows show a
code call or a Cargo dependency. Blue arrows show runtime data flow. Dashed red
arrows show build-time generation or packaging.

## Diagrams

- `00-overview.drawio`: all crates, packages and tools, grouped by layer, with
  provenance-web as an external renderer that the CLI serves from pinned assets.
- `01-core-and-macros.drawio`: provenance-core modules, provenance-macros, and
  which crates import them.
- `02-store-writes.drawio`: provenance-store write path, lock order, and the
  canonical JSONL files in `.provenance/state/`.
- `03-store-cache-and-reads.drawio`: provenance-store reads, freshness policy,
  SQLite catch-up, and reads that use git.
- `04-transport-and-contract.drawio`: provenance-transport, provenance-porcelain,
  and the `operations::catalog` contract that HTTP, MCP and in-process calls share.
- `05-cli-dispatch.drawio`: how provenance-cli parses a command and sends it to
  the in-process host or to a built-in handler.
- `06-agent-surfaces.drawio`: init guidance in AGENTS.md, skill install, prime,
  dogfood notes, and tools/agent-notifications.
- `07-review-host.drawio`: `provenance review`, the launch flow, review events,
  and how tools/review-host builds page assets from the pinned provenance-web
  archive.
- `08-scanner-and-coverage.drawio`: provenance-scanner stages and the callers of
  its functions.
- `09-report.drawio`: provenance-report envelope build and render.
- `10-ste100.drawio`: provenance-ste100 checks, the project dictionary, and the
  store functions that call them.
- `11-codegen-and-clients.drawio`: provenance-codegen, tools/operation-codegen,
  the generated output, and provenance-http-client.
- `12-sdks.drawio`: the TypeScript SDK, create-provenance, and provenance-sdk.

## Merged and split subsystems

- provenance-macros is in diagram 01 with provenance-core.
- provenance-porcelain is in diagram 04 with provenance-transport.
- provenance-http-client is in diagram 11 with the generator.
- packages/create-provenance is in diagram 12 with the SDKs.
- provenance-store has two diagrams (02 for writes, 03 for reads and the cache).

## What the code did not show

- No CLI command calls `StatementHost::serve_mcp`. Only tests use it, so the
  diagrams show the MCP server with no production caller.
- `publication::source_edit` has no production caller. Only tests use it.
- The internals of provenance-web are not in this repository. The diagrams show
  only the pin in `tools/review-assets.json` and the `mountReview` entry point.
- `skills/` and `crates/provenance-cli/skills/` hold the same four SKILL.md
  files (the git blobs are identical). The code embeds only the crate copy. No
  script or test that keeps the two copies the same was found.
- The path `.provenance/cache/scopes/<scope>/verification-runs.jsonl` comes from
  `layout.rs`. No run file was seen on disk.
- No workflow in this repository runs `provenance report build` or
  `provenance report render`. The caller of the report in CI is not known.
- After e93799a4, main gained three commits (#482, #483, #484). They change the
  launch code lifetime, the withdrawal of a rejected candidate, and the manifest
  data. They do not change a box or an arrow in these diagrams.
