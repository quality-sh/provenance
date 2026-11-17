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

Each entry of the manifest field `disposition_actor_ids` names an identity type
and an ID, in the same form as the actor of a review decision:
`{"identity_type":"human","id":"reviewer-id"}`. The init command writes a
`human` entry for each `--disposition-actor-id`, and the flag replaces the
list. When the list holds an `agent` or `service` entry, init with
`--disposition-actor-id` is refused and names each such entry; edit the field in
`.provenance/state/manifest.json` to change it. A review decision is recorded
only when its actor has the identity type and the ID of one entry; the name of
the actor has no effect. The author of a review submission, or an actor that
matches an entry, can withdraw the submission. A submission from a write that
gives no actor records the actor `authoring` and has no author, so only an
actor that matches an entry can withdraw it. A withdrawal by each other actor
gets 403 `access_denied`. An entry in the text form, such as `"reviewer-id"`,
makes each read and write of the repository fail with `invalid_state_file`,
and the message gives the object form of the entry.

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
`rule`, `topic`, `question`, and `domain`.

A word that names an API collection, such as `rules` in `provenance rules list`,
is not a command. The CLI refuses it with a usage error and shows
`provenance --help`.

A Requirement `create` or `update`, `discuss`, and `reply` take an optional
`--actor`. A Requirement `create` or `update` or a `discuss` that gives no actor records
the actor `authoring`, on the CLI, on MCP, and on the HTTP API. A `reply` stores no actor. A
JSON `null` actor is invalid input. A Rule or Resolution write takes no actor and
records `authoring`. `submit`, `claim`, and `release` need `--actor`.

Rule creation defaults to `draft` in the CLI and in catalog operations. An
update with `--status active` or `--status archived`, or a create with
`--status active`, opens a status request and does not change the status. A
create with `--status archived` is refused: create the Rule, then request the
archive. A create with `--status active` writes the Rule as `review` until a
reviewer accepts the request. Coverage reports do not produce active-Rule
absence findings for a draft Rule.

A status request of a Rule or a Requirement is a review submission. The result
of the request gives its Proposal ID under `decision.pending.proposal_id` and
the call that accepts it. A reviewer on the reviewer list of the repository
accepts it with `provenance api <collection>/<id>/submissions/<proposal_id>/decide
--method post --input -`, where `<collection>` is `rules` or `requirements`. The
input names the reviewer, the decision and the reviewed record:
`{"actor":{"identity_type":"human","id":"<reviewer>"},"decision":"accepted","canonical_artifact":{"artifact_type":"rule","artifact_id":"<id>"}}`.
An acceptance with the `agent` identity type needs a prior assertion, which a
review submission does not have, so it is refused with
`approval_evidence_required`. The status changes in the write of the
acceptance. A rejection or a withdrawal keeps the status. With an empty
reviewer list, nobody can accept the request: the result says so and names
`provenance init --disposition-actor-id <id>`, which adds a reviewer.

A typed specification cannot set a Rule status, so apply creates each new
specification Rule as `draft`. To make a specification Rule active, request the
status with `provenance <id> update --status active --declared-by <owner>
--if-match <etag>`. The Rule is active when an allowed reviewer accepts the
request. A later apply keeps the accepted status. If a
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
`--if-match`, which must contain `edit.etag` from the latest record read, or
`review.edit.etag` from the latest review read, unchanged. Requirement, Topic,
and Question updates require it. Source, Rule, and Resolution updates accept
it, and write without a check when it is absent. No update accepts the guard
as an `--expected-etag` flag or as an `expected_etag` body field.

`provenance api` shows at most 50 routes by default. Use `--filter <text>` to
select routes or `--limit <number>` to increase the bound. With `--format json`,
the answer has no default bound. An answer that lists more than one route gives
the method, path, description, and inputs of each route, and no schemas. To see
the request and response schemas of one route, give its method and path as the
filter, for example `provenance api --filter "GET /rules/{id}"`. The text answer
and the `--format json` answer of one route both give its schemas.

Every catalog result uses `{data,meta}` or `{error,meta}`. Lists put their
records in `data.items`.

With `--format json`, each command that takes `--format` prints a refusal as
one `{error,meta}` object. The [operation contract](operation-contract.md#envelopes-and-failures)
defines the object and the kinds `operation_failed`, `statement_invalid`, and
`invalid_input`. The object is one line on standard error with no prefix, and
the command exits with code 1. A `hint:` or `warning:` line can come before it.
When the CLI knows the next command to run, `error.hint` names it.

Only a `--format json` that the caller gives selects the object. These commands
print JSON by default, and without the flag they print a refusal as text:
`export`, `merge-jsonl`, `validate`, `schema show`, `dictionary status`,
`skills list`, `skills install`, `report build`, `dogfood list`,
`dogfood report`, and `dogfood triage list`.

Without `--format json`, a refusal is text after `Error: ` with exit code 1.
`provenance api` prints its refusal object after `Error: `. An option that the
action does not take, or an input value that the command cannot read, is a
usage error: clap's text with exit code 2.

When the parser refuses the command line, the refusal is clap's text with exit
code 2, also with `--format json`.

## Product commands

The following commands keep dedicated behavior because they operate on a
repository, render a report, manage the host, or perform another product task:

- `init` and `check`
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

## JSONL merge driver

State is one record per line, so the line merge of git makes conflicts when two
branches change different records. `provenance merge-jsonl` merges state files
by record id. `provenance init` sets this up:

- It adds this line to `.gitattributes`, unless the file has that line:

  ```
  .provenance/state/**/*.jsonl merge=provenance-jsonl
  ```

- Git does not keep the command of a merge driver in the repository, so init
  also sets `merge.provenance-jsonl.driver` in the local git configuration of
  the clone. The value is the command that runs Provenance in the project,
  followed by `merge-jsonl %O %A %B --output %A --path %P`. For a native
  install the command is `provenance`, which git finds on `PATH`. When a git
  configuration level already sets the driver, init keeps that value.

Run `provenance init` one time in each clone. In a new clone it changes no file
and sets only the driver. Outside a git work tree, or when git does not run,
init writes `.gitattributes` and prints a notice. Git runs the driver from the
top folder of the work tree. A TypeScript project in a subfolder has a command
that does not run from there, so init does not set the driver and prints the
command to set by hand.

`provenance check --graph` shows a finding when git sends state files to the
driver and the clone has no command for it. The finding does not change the
exit code. Without the driver, git merges state files line by line.

The placeholders are the full contract of the command. `%O %A %B` are the
temporary base, ours, and theirs files. `--output %A` writes the merged records
over the ours file, which git reads. `--path %P` is the path of the file in the
repository, which tells the merge the record kind. The command exits non-zero
when the merge has a conflict or when the merged records fail the write checks;
git then leaves the path unmerged. The JSON report names each conflicting
record, its kind (`add_add`, `divergent_edit`, or `delete_modify`), and the
base, ours, and theirs versions.

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
- `materialize`, `import`, `dictionary_import`, and `swarm_backtrace_land`
  write the repository files, as `provenance materialize`, `provenance import`,
  `provenance dictionary import`, and `provenance swarm-backtrace land` do. The
  server lists them when it serves a repository, as `provenance mcp` does.

The annotations of `change` mark it as destructive, not idempotent, and
open-world: an update can replace field values, remove list entries, and archive
a record, a repeated `discuss` starts a new Discussion, and `notify` reaches a
client thread outside the repository.

`api` is the plumbing tool. It reads and writes, as `provenance api` does: it
lists the catalog routes when `path` is absent, or calls one route with its
`method`, `query`, `headers`, and `body`. A listing gives at most 50 routes
without schemas; `{"filter":"GET /rules/{id}"}` gives that one route with its
request and response schemas, in the structured answer and in the text. Its annotations mark it as
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

The `hint` of the refusal names this section.

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

## Repairing review rows that do not agree

The review rows of a record are its review submissions, the decisions on them,
and their withdrawals. They are in `ideation/proposal_cards.jsonl`,
`ideation/dispositions.jsonl`, and `ideation/withdrawals.jsonl`. A merge of two
branches keeps the rows that each branch added, and a person can keep the two
sides when Git cannot merge a file.

Rows that say the same thing read as one row: one row ID on two equal lines,
two decisions on one submission with the same result, and two withdrawals of
one submission. Rows that do not agree make each read and each review write of
their record refuse with `invalid_state_file`:

- One row ID is on more than one line, and the lines do not have equal
  contents.
- The decisions on one submission have different results.
- One submission has an approval and a withdrawal.
- More than one submission of one record has the same review cycle.

Each line takes part in each of these tests, also a line of an ID whose lines
do not agree. Thus the decisions and the withdrawals that name such a
submission, and the review cycle of each of its lines, give their faults in the
same read. A write that needs the approval of
the record gives the same refusal: an active Rule or an answered Question that
cites a Resolution that is not approved needs the approval of that Resolution.

The refusal names the file and the line of one row. Its message names the rows
and their lines, and its `hint` gives the repair step, which names each line
that the repair removes. The text form gives the message, then `; ` and the
repair step. For example:

```text
scopes/default/ideation/dispositions.jsonl line 2: review submission S of requirement req_a has decisions with different results in ideation/dispositions.jsonl: accepted D1 at line 1 and rejected D2 at line 2; To repair, remove each of these lines that has a result that you do not keep, then commit.
```

This refusal does not give the two general procedures of a state file that
does not parse, because it has a repair step of its own.

Other records, the documents that do not hold the record, and the reads of all
records of a type continue. `provenance check` reports each of these faults
with its file and its line.

A withdrawal that names no review submission makes each review read of the
scope refuse. Its message names the withdrawal, and its repair step names its
line.

To repair the rows, do the repair step that the refusal gives, then commit.
Some steps give a choice: keep one line of an ID, keep the decisions of one
result, keep the approval or the withdrawals, or keep one submission. Remove
only the lines that the step tells you to remove. One run of `provenance check`
reports each fault of the record, and a repair step does not show a new fault.
After the repair step of each fault, the reads of the record and
`provenance check` pass. This is the only hand edit of
`.provenance/state` that the generated AGENTS.md permits.

## Agent guidance

`provenance prime` introduces the Provenance domain. It explains Requirements,
Rules, Resolutions, Sources, and code bindings. It does not read project records,
inspect installed skills, scan code, or update a cache. Its list of actions has one
line for each command that `provenance --help` lists, including `review` and `mcp`.
The line gives each form that selects the command, for example `<id> update` for an
action on a record, and `discussions or <id> discussions` for a command that also
acts on a record.

The default output is Markdown. `provenance prime --format json` returns an
object with a `guidance` string that contains the same text. This replaces the
former project-state JSON fields, including `rules`, `requirements`, `threads`,
and `skills`. Use resource reads for project records and `check --bindings` for
code bindings. The SDK prime state query keeps its existing contract.

The old `--repo`, `--scope`, and `--include-threads` options remain accepted for
compatibility but have no effect on guidance. MCP supplies the same domain
content in server instructions and shared tool descriptions. The instructions
have one line for each tool and each action that the server lists, with its MCP
name, for example `dictionary_import`. The `notify` action is not a command: it has
no line of its own, and the `change` line names it. MCP has no prime tool. Each MCP client
controls how it presents server instructions.

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
