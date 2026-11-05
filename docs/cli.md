# CLI

Install `provenance` from a release archive, with `cargo install
provenance-cli`, or through the `@quality-sh/provenance` development package.
The native distribution also contains `cargo-provenance`.

Initialize a repository before you use resource commands:

```sh
provenance init --path . --scope default --path-prefix .
```

On a terminal, `init` asks for the human reviewer ID. The prompt uses the Git
email address or name as its default when one is available. In a
non-interactive run, `init` uses that default automatically. It converts the
default to lowercase and replaces other character groups with underscores.

To replace the reviewer list after initialization, run:

```sh
provenance init --path . --disposition-actor-id reviewer-id
```

Repeat `--disposition-actor-id` to permit more than one reviewer. Use
`--clear-disposition-actors` to remove all reviewers. If the reviewer list is
empty, `provenance review` permits reads but not review decisions.

## Record commands and the api command

The CLI reads and changes one record with the record ID as the first word.
Plumbing operations of the catalog have no commands of their own: call them
with `provenance api <path>`. The same catalog defines HTTP routes, the MCP
`api` tool, OpenAPI operations, and generated clients.

Global `--repo`, `--scope`, and `--format` flags can occur before or after the
command words. The default repository is the current directory. The default
scope is `default`.

A record command uses the record ID:

```text
provenance <id> create --type <kind> [scalar flags | --stdin]
provenance <id> get [--view record|children|trace|impact|review]
provenance <id> trace [--depth <n>] [--kind <kind>] [--limit <n>]
provenance <id> update [scalar flags | --stdin] [--if-match <etag>]
provenance <id> answer|claim|release|submit [flags]
provenance <id> discussions|discuss
provenance search --text <text> | --kind <kind> | --file <path> [--symbol <name>] [--line <line>]
provenance check --statement "<statement>"
```

The `children` view gives the records below a record. The `trace` view gives
the records above it, up to its Sources. `provenance <id> trace` gives the same
records as `provenance <id> get --view trace`, and MCP has a `trace` tool and a
`trace` view of the `get` tool.

`provenance <id> create --type <kind> --help` lists the fields that one record
type accepts. The record types are `source`, `requirement`, `resolution`,
`rule`, `topic`, `question`, `domain`, and `boundary`.

A word that names an API collection, such as `rules` in `provenance rules list`,
is not a command. The CLI refuses it with a usage error and shows
`provenance --help`.

Rule creation defaults to `draft` in the CLI and in catalog operations. Pass
`--status active` only when a person ratifies the Rule. Coverage reports do not
produce active-Rule absence findings for a draft Rule.

A typed specification cannot set a Rule status, so apply creates each new
specification Rule as `draft`. To make a specification Rule active, a person
changes its status with `provenance <id> update --status active
--declared-by <owner> --if-match <etag>`. A later apply keeps that status. If a
Rule declaration changes its key or moves to a different Requirement, apply
creates a new `draft` Rule and deletes the old Rule. A move between a
Requirement and the shared specification context keeps the Rule and its status
when apply finds one existing Rule.

Search hides archived and other terminal records by default. Use
`--include-terminal` to include them.

Read the current review decision, the rejection comment, and bounded Discussion
messages:

```sh
provenance <record-id> get --view review
```

The result shows `review.edit.etag` and the exact `--if-match` value for the next
guarded update. A guarded update after a rejection opens the replacement
submission. Do not submit or withdraw manually.

For one bounded view across record kinds, use the target-first read:

```text
provenance <id> get --view children --depth <count>
```

Examples:

```sh
printf '%s' '{"name":"Policy","source_type":"document","supersedes":[]}' |
  provenance source_policy create --type source --stdin
provenance search --text "time bounded" --kind rule --limit 20
provenance topic_open claim --actor agent-a
provenance topic_open release --actor agent-a
provenance question_open answer --answer "Use the guarded path."
```

Other catalog routes take their path, method, and body through `api`:

```sh
provenance api sources
provenance api rules --query query=stale --query base=main
provenance api requirements/req_review/document --query limit=50
provenance api requirements/req_review/history
provenance api rules/rule_shared --query query=trace --query direction=out
provenance api proposals/proposal_1/assertions --method post --input assertion.json
provenance api verification-runs/run_1/complete-verification --method post --input result.json
```

`--input` reads the JSON body from a file, or from standard input with `-`. The
body is the data object without the `{"data": ...}` wrapper. `--query` and
`--header` take `name=value` and `Name: value`.

Scalar body fields of a record command use flags. The CLI converts Boolean,
integer, floating-point, null, and string values to their catalog types, and a
string-typed field keeps the exact text it receives. A field backed by an array
accepts one item per use, so repeated flags build the array. A
`--<field>-json` flag carries one whole JSON value for a field, such as
`--supersedes-json '[]'`; a JSON null clears the field where the schema allows
null, while a plain flag on a string-typed field always supplies text. Arrays
and objects can also come from one JSON object on standard input with
`--stdin`, which fills only the body fields that no flag assigned and refuses a
field that a flag already set. Every review record update takes its guard as
`--if-match`, which must contain `review.edit.etag` from the latest review
read, unchanged. Requirement, Topic, and Question updates require it. Source,
Rule, Resolution, Domain, and Boundary updates accept it, and write without a
check when it is absent. No update accepts the guard as an `--expected-etag`
flag or as an `expected_etag` body field.

`provenance api` shows at most 50 routes by default. Use `--filter <text>` to
select routes or `--limit <number>` to increase the bound. Use `--format json`
only when you need all request and response schemas.

Every catalog result uses `{data,meta}` or `{error,meta}`. Lists put their
records in `data.items`.

With `--format json`, a record read, search,
or action, and a Discussion command print a refusal from the operation as one
`{error,meta}` object. The object is the only line on standard error, it has no
prefix, and the command exits with code 1. When the CLI knows the next command
to run, `error.hint` names it.

`provenance api` prints a refusal as one `{error,meta}` object after `Error: `
on standard error and exits with code 1.

These errors stay text after `Error: ` and exit with code 1, also with
`--format json`:

- A graph that is not initialized, and a command that runs in a directory
  inside an initialized repository.
- File and I/O errors.
- Every error from a builtin command, such as `init`, `check`, and `export`.

A usage error prints clap's text and exits with code 2.

## Product commands

The following commands keep dedicated behavior because they operate on a
repository, render a report, manage the host, or perform another product task:

- `init`, `check`, and `docs`
- `prime`
- `review` and `mcp`
- `import`, `export`, and `materialize`
- `swarm-backtrace`
- `skills`, `schema`, and `validate`
- `dogfood` in development builds

`provenance check --format json` reports diagnostics for new Requirement and
Rule statements and for statements that differ from Git HEAD. Add `--strict`
to reject findings in the committed candidate and every bindings finding. Use `--base <commit>` to select
the comparison base.

`provenance check --gaps` runs only the gaps category. It reports the
unfinished parts of the graph and each Rule without a Source: one finding for each gap and one for each such
Rule. The graph counts of each scope are in the category context, also when
there are no findings. These findings do not fail the check, also with
`--strict`. A bare `check` does not run this category. MCP `check` takes
`gaps` in `categories`. `--scope <scope>` limits each category to one scope;
without it, `check` reads every declared scope. `gaps`, `health`, and
`orphans` were removed: they exit with code 1 and name
`provenance check --gaps`.

`provenance check --bindings` scans Rule implementation and verification
bindings in every scope, with Rule validation. The configured severity in
`.provenance/settings.json` decides which findings fail the check: `error`
fails it for the governed findings, `warning` only reports them. `--strict`
fails it for every bindings finding. These options apply only to the bindings
check, and `check` refuses them without it:

- `--path <path>` scans the files below the path. The default is the
  repository root. A path that does not cover a scope reports no absence
  findings for that scope.
- `--baseline <file>` resolves evidence anchors against an earlier scan report.
  It needs `--scope`.
- `--output <file>` writes the scan report of the selected scope to the file,
  in the report shape of the removed `coverage scan`. It needs `--scope`.
  `--output-format markdown` writes the Markdown report; the default is JSON.
  Standard output keeps the check report.

`coverage scan` was removed: it exits with code 1 and names
`provenance check --bindings`. `graph` and `traceability` were removed the same
way: they name `provenance <id> get --view children` and
`provenance <id> trace`.

`provenance help [command]` prints the same help as
`provenance [command] --help`. Help does not show `merge-jsonl`, which Git runs
as a merge driver; the command still runs.

One Discussion is read with `provenance <discussion-id> discussion`.
`provenance discussions` lists Discussions; `provenance discussions
<discussion-id> get` was removed and names the discussion action.

The review host binds one repository, one scope, and one credential to each
connection. See [review-host.md](review-host.md). Import and export operate on a
complete scope. They remain separate from resource writes.

## MCP server

`provenance mcp --repo <repository> --scope <scope>` starts an MCP server on
standard input and output. The server opens the same repository and scope as the
CLI commands. The default repository is the current directory, and the default
scope is `default`. The server uses MCP protocol 2026-07-28. A client of an
older protocol, from 2024-11-05 through 2025-11-25, starts the session with
`initialize`.

The server gives seven tools. Five tools only read, and their annotations mark
them as read-only and closed-world:

- `get` reads one record in a view.
- `search` finds records.
- `trace` reads the records above one record, up to its Sources.
- `check` checks the graph or one statement.
- `discussions` lists Discussions (`action` is `discussions`, the default) or
  reads one Discussion (`action` is `discussion`).

`change` writes. Its `action` argument selects the write:

- `create`, `update`, `answer`, `claim`, `release`, or `submit` on the
  `target` record, with the same fields as the CLI action after the target.
- `discuss` starts a Discussion, and `reply` replies to one.
- `notify` registers a client thread for the review changes of one record, as
  `--notify` does on the CLI.

The annotations of `change` mark it as destructive, not idempotent, and
open-world: an update can replace field values, remove list entries, and archive
a record, a repeated `discuss` starts a new Discussion, and `notify` reaches a
client thread outside the repository.

`api` is the plumbing tool. It reads and writes, as `provenance api` does: it
lists the catalog routes when `path` is absent, or calls one route with its
`method`, `query`, `headers`, and `body`. Its annotations mark it as
destructive, not idempotent, and closed-world: a route can change or remove
data in the repository.

Each tool input is one JSON object. The server refuses a call whose fields do
not agree with its `action`. The refusal is a tool result with `isError: true`,
and its error names the field: the field that the action needs, or the field
that the action does not take. A `change` call without `data` gets the schema
of `data` for that action and record kind in the refusal.

A record that the CLI writes is available to the MCP tools in the same session,
and a record that an MCP tool writes is available to the CLI. A write through
MCP goes into the graph like a CLI write, and Provenance review approves the
graph content.

The server stops when standard input closes. A write that started before then
completes before the process stops. If the repository has no Provenance graph,
the command writes the refusal to standard error and stops with a nonzero exit
status.

To add the server to an MCP client, give the client the command and its
arguments. For example, a client that reads an `mcpServers` object:

```json
{
  "mcpServers": {
    "provenance": {
      "command": "provenance",
      "args": ["mcp", "--repo", "/path/to/repository"]
    }
  }
}
```

### Allow rules for Claude Code

Claude Code asks before each MCP tool call until a permission rule allows the
tool. The tool name is `mcp__<server>__<tool>`, where `<server>` is the name in
the client configuration. With the name `provenance`, put one of these lists in
`permissions.allow` of a Claude Code settings file.

To allow all tools, writes included:

```json
["mcp__provenance__*"]
```

To allow the read tools only, so that each `change` and `api` call asks first:

```json
[
  "mcp__provenance__get",
  "mcp__provenance__search",
  "mcp__provenance__trace",
  "mcp__provenance__check",
  "mcp__provenance__discussions"
]
```

Claude Code does not use `readOnlyHint` to skip the request, so the allow rule
is necessary also for the read tools. `provenance init` does not write client
settings.

Codex runs the read tools without a request, because their annotations mark
them as read-only. To also allow `change` and `api`, add this to the Codex
configuration:

```toml
[mcp_servers.provenance.tools.change]
approval_mode = "approve"

[mcp_servers.provenance.tools.api]
approval_mode = "approve"
```

## Repairing a duplicate Message ID

New Message IDs use `msg_` followed by a random UUID. An existing store can
contain duplicate IDs from writes on separate branches. A Message read refuses
that store and names the duplicate ID and all its shard lines:

```text
Message ID msg_000021 appears more than once: <shard>:<line>, <shard>:<line>
```

To repair the store by hand:

1. Stop writes to the repository. Make a backup of `.provenance/state`.
2. Read each named row. Use its body and Thread to identify the Discussion that
   owns it. Two Discussions can share a Thread. Do not use the Thread alone to
   select the owner, and do not delete either Message.
3. Choose a fresh `msg_<uuid>` ID for one duplicate row. Replace that row's `id`
   in the named Message shard. Keep its other fields, including `created_at`.
4. In the same scope's `threads/discussions.jsonl`, replace the old ID in the
   owning Discussion's `message_ids`. If this is its root Message, also replace
   `root_message_id`. Update any `outcomes[].message_id` or graph-record
   `origin_message` reference that belongs to the renamed Message. Keep the
   references to the other duplicate row unchanged. If ownership is unclear,
   use the branch history to identify it before you change a reference.
5. Review the diff. Run `provenance check --repo <repository>`, then
   `provenance discussions --repo <repository>` and
   `provenance <discussion-id> discussion --repo <repository>` for each
   affected Discussion. Repeat the repair for any other duplicate ID that the
   reader reports. Commit the repaired shards when the checks pass.

There is no automatic repair command. The duplicate rows do not always contain
enough information to select the owning Discussion.

## Agent guidance

`provenance prime` introduces the Provenance domain. It explains Requirements,
Rules, Resolutions, Sources, and code bindings. It does not read project records,
inspect installed skills, scan code, or update a cache.

The default output is Markdown. `provenance prime --format json` returns an
object with a `guidance` string that contains the same text. This replaces the
former project-state JSON fields, including `rules`, `requirements`, `threads`,
and `skills`. Use resource reads for project records and `check --bindings` for
code bindings. The SDK prime state query keeps its existing contract.

The old `--repo`, `--scope`, and `--include-threads` options remain accepted for
compatibility but have no effect on guidance. MCP supplies the same domain
content in server instructions and shared tool descriptions. MCP has no prime
tool. Each MCP client controls how it presents server instructions.

## Dictionary setup

Statement checks use the project dictionary when the local dictionary index is
available. A repository commits `.provenance/state/dictionary.json`; the index
content stays in the machine data directory. Run:

```sh
provenance dictionary import --pdf /path/to/ASD-STE100-Issue-9.pdf
provenance dictionary status
```

Cache the machine data directory in CI. A strict statement check fails when the
repository declares a dictionary but the selected index is unavailable.
