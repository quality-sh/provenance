# Dogfood notes (dev builds only)

`provenance dogfood` is an internal feedback channel: agents working with
provenance record structured pain-point notes about the tool itself. It is
compiled in only when the `dogfood` cargo feature is enabled and must never
ship in a released binary (see [release.md](release.md)).

## Security model

Capture is **local-only**. `dogfood note` appends to a JSONL spool on the
machine it runs on — there is no network path, no endpoint, and nothing to
authenticate. Someone who compiles the feature themselves gets a command that
writes to their own disk. Collection across machines, if ever needed, happens
out-of-band over infrastructure the operator already trusts (e.g. a
tailnet), never through this CLI.

## Capture

```sh
provenance dogfood note \
  --surface prime --category friction --severity annoyance \
  --detail "Had to run three follow-up queries to see full rule statements." \
  "prime output truncates rule statements"
```

The agent supplies three low-cardinality dimensions plus text:

| field | values |
| --- | --- |
| `--surface` | any top-level provenance subcommand name, or `general` (validated against the real command tree) |
| `--category` | `friction` / `confusion` / `missing` / `bug` / `idea` |
| `--severity` | `blocked` / `workaround` / `annoyance` — impact on the task at hand |
| summary (positional) | one line |
| `--detail`, `--suggestion` | optional free text |

Everything else is stamped by the CLI, never self-reported: `ts_ms`, `host`,
repo/branch/commit (null outside a git repo), `provenance_version`, and
`session_id` — read from the first non-empty of `PROVENANCE_SESSION_ID`,
`WORKFLOWD_SESSION_ID`, `CLAUDE_SESSION_ID`, `OPENCODE_SESSION_ID`. Missing
context degrades rather than failing capture: session/git fields go null,
`host` falls back to `"unknown"`, and with no resolvable home directory the
spool degrades to `$TMPDIR/provenance-dogfood`.

Notes land in `~/.provenance/dogfood/notes.jsonl` (override the directory
with `PROVENANCE_DOGFOOD_DIR`; an empty value is treated as unset, and a
relative value resolves against each invocation's working directory), one
JSON object per line, append-only. Readers skip unparseable lines with a
warning, so a torn write never blocks `list`/`report`.

## Review

```sh
provenance dogfood list --format json
provenance dogfood report --format json
provenance dogfood report --enrich sessions.json
```

`report` aggregates counts by `surface` × `category` × `severity` and emits
the full notes.

## Enrichment contract: `provenance-dogfood-enrichment/v1`

The note deliberately carries only a session-id join key. Ground truth about
the session — harness, model, machine — lives in whatever sister system
launched the agent (e.g. workflowd). That system is **never a dependency**:
provenance neither links it, queries its database, nor calls its API. The
only coupling is a shape. Any producer that can emit this JSON can enrich a
report:

```json
{
  "contract": "provenance-dogfood-enrichment/v1",
  "sessions": {
    "<session_id>": {
      "harness": "opencode",
      "harness_version": "1.2.3",
      "model": "claude-opus-5",
      "machine": "mint",
      "agent": "build",
      "repository": "owner/repo"
    }
  }
}
```

- Keys of `sessions` match the `session_id` values stamped into notes
  (harness-native session ids).
- All fields inside a session object are optional and passed through
  verbatim to the report, so producers can add fields without a provenance
  release.
- A `contract` value other than `provenance-dogfood-enrichment/v1` is
  rejected.

Feed it via file or stdin:

```sh
curl -s http://mint:PORT/dogfood/sessions | provenance dogfood report --enrich -
```
