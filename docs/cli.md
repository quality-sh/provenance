# CLI

Install `provenance` from a release archive, with `cargo install
provenance-cli`, or through the `@quality-sh/provenance` development package.
The native distribution also contains `cargo-provenance`.

Initialize a repository before you use resource commands:

```sh
provenance init --path . --scope default --path-prefix .
```

## Resource commands

The CLI derives its resource dialect from the operation catalog. The same
catalog defines HTTP routes, MCP tools, OpenAPI operations, and generated
clients.

Global `--repo`, `--scope`, and `--format` flags can occur before or after the
resource words. Catalog commands support JSON output. The default repository
is the current directory. The default scope is `default`.

The collection commands are:

```text
provenance <collection> list
provenance <collection> create [scalar flags | --stdin]
provenance <collection> search --text <text> [--limit <count>] [--cursor <cursor>]
provenance rules stale [--base <commit>] [--head <commit>]
provenance rules resolve-symbol --file <path> [--symbol <name>] [--line <line>]
```

The writable collections are `sources`, `requirements`, `resolutions`,
`rules`, `domains`, `boundaries`, `topics`, `questions`, `contributions`,
`synthesis-packets`, and `proposals`. The catalog also exposes reads for
`verification-runs`, `verification-bindings`, `discussion-containers`,
`messages`, `assertions`, and `dispositions`.

A member uses its ID in the command address:

```text
provenance <collection> <id> get
provenance <collection> <id> update [scalar flags | --stdin]
provenance <collection> <id> trace [--direction in|out|both] [--max-depth <count>]
provenance <collection> <id> neighbors [--direction in|out|both] [--limit <count>]
provenance <collection> <id> impact
```

Examples:

```sh
provenance --repo . --scope default sources list
printf '%s' '{"id":"source_policy","name":"Policy","source_type":"document","supersedes":[]}' |
  provenance sources create --stdin
provenance rules search --text "time bounded" --limit 20
provenance rules rule_shared trace --direction out --max-depth 2
provenance topics topic_open claim --actor agent-a
provenance questions question_open answer --answer "Use the guarded path."
```

Parent-owned resources keep their parent address:

```sh
provenance requirements req_review document get --limit 50
provenance requirements req_review history get
provenance requirements req_review history entry_1 evidence before get --field statement
provenance requirements req_review submit --idempotency-key request_1 --stdin
provenance requirements req_review submissions proposal_review decide --idempotency-key request_2 --stdin
provenance sources source_policy discussions get
provenance sources source_policy discussions create --idempotency-key request_3 --stdin
provenance sources source_policy discussions discussion_1 messages create \
  --idempotency-key request_4 --if-match '"1"' --stdin
provenance proposals proposal_1 assertions create --stdin
provenance verification-runs begin-verification --stdin
provenance verification-runs run_1 complete-verification --status passed
```

Scalar body fields use flags. The CLI converts Boolean, integer, floating-point,
null, and string values to their catalog types. Arrays and objects use one JSON
object on standard input with `--stdin`. Do not combine body flags with
`--stdin`. Path identity comes from the command address. Query parameters and
the `Idempotency-Key` and `If-Match` controls use the flags that the catalog
declares.

Every catalog result uses `{data,meta}` or `{error,meta}`. Lists put their
records in `data.items`.

## Product commands

The following commands keep dedicated behavior because they operate on a
repository, render a report, manage the host, or perform another product task:

- `init`, `check`, and `docs`
- `graph`, `traceability`, `coverage`, `gaps`, `health`, `orphans`, and `prime`
- `review`
- `import`, `export`, and `materialize`
- `swarm-backtrace`
- `skills`, `schema`, and `validate`
- `dogfood` in development builds

`provenance check --format json` reports diagnostics for new Requirement and
Rule statements and for statements that differ from Git HEAD. Add `--strict`
to reject findings in the committed candidate. Use `--base <commit>` to select
the comparison base.

`provenance coverage scan --path . --scope default --validate-rules` scans Rule
implementation and verification bindings. Add `--strict` to make warnings
return a nonzero exit status.

The review host binds one repository, one scope, and one credential to each
connection. See [review-host.md](review-host.md). Import and export operate on a
complete scope. They remain separate from resource writes.

## Agent guidance

`provenance prime` introduces the Provenance domain. It explains Requirements,
Rules, Resolutions, Sources, and code bindings. It does not read project records,
inspect installed skills, scan code, or update a cache.

The default output is Markdown. `provenance prime --format json` returns an
object with a `guidance` string that contains the same text. This replaces the
former project-state JSON fields, including `rules`, `requirements`, `threads`,
and `skills`. Use resource reads for project records and `coverage scan` for
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
