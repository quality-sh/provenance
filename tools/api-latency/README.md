# API latency

The `API latency` workflow (`.github/workflows/api-latency.yml`) times each
local API call of a release build. Start it from the Actions page with
workflow_dispatch. It does not run on a pull request or a push.

## What it times

- Each operation of the catalog: each route with each of its query variants,
  reads and writes. The list comes from `provenance api --format json`, so a
  new route is timed with no change to the benchmark.
- Each porcelain command from `provenance --help` and its subcommands, and each
  action of `provenance <id> <action>`.
- Each MCP tool and each action of a tool, from `tools/list`.

## Ways in

- HTTP: requests to one `provenance review` host, the way the review page
  calls it.
- CLI: one `provenance` process for each call: `provenance api <path>` for the
  catalog routes, and the porcelain commands.
- MCP: tool calls over standard input and output to one `provenance mcp`
  process. The catalog routes go through the `api` tool.

## Input

The run clones the dispatched commit, so the input is the
`.provenance/state` of that commit. When the state has no record for a read
route, for example no Discussion, the run makes one with the catalog writes
and the report names it. Each way in then copies that clone, so the writes of
one way do not change the input of another.

## Timing

- Cold is the first call after the process starts. Before each HTTP or MCP
  call row, the host restarts.
- Warm is the median (p50) and the nearest-rank p95 of the repeats after the
  cold call: 20 for reads and 10 for writes by default.
- For each write, the read of the record that the write changed is its own
  row (`..., then the first read`). This read includes the projection catch-up.
- A write repeat gets new records to act on before it. That setup is not
  timed.

A call that the run cannot make is not timed. The report lists it with the
reason, for example a route for which the state has no record, or a write
that the store refuses.

## Files

- `run.mjs` runs the calls and writes `results.json`. With `--windows <file>`,
  it also writes the process id and the time window of each timed call. The
  API profile workflow uses that file (see `tools/profile/README.md`).
  `--route-writes MCP` calls the catalog write operations through MCP only;
  the default is `HTTP,MCP,CLI`.
- `report.mjs` ranks the rows, slowest warm p50 first, and writes `report.md`
  and `report.json`. A row is marked when its cold, p50 or p95 time is over
  200 ms.
- `catalog.mjs`, `requests.mjs`, `body.mjs` and `commands.mjs` find the calls
  and their input. `ways.mjs` holds the three clients.

Run the unit tests with `node --test tools/api-latency/*.test.mjs`.
