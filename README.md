# Provenance

Never lose the *why* behind your decisions.

Provenance is a tool for building requirements traceability, from source to requirement to rule to the code and tests that enforce them.

### Installation

```sh
cargo build --release -p provenance-cli --features scanner
```

The binary lands at `target/release/provenance`. Put it on your PATH.

### Quick start

```sh
# set up a repo (commit .provenance/state/, ignore .provenance/cache/)
provenance init --path . --scope default --path-prefix .

# put something in the graph
provenance requirements create --scope default --id req_exports \
  --statement "Exports finish in under a minute"

# see where things stand
provenance prime
```

### Essential commands

| Command | What it does |
| --- | --- |
| `provenance prime` | Low-res view of the graph, the right thing to feed an agent at the start of a session |
| `provenance check` | Validate the state files |
| `provenance materialize` | Rebuild the SQLite query cache |
| `provenance graph <requirement>` | Show the neighbourhood of a requirement |
| `provenance traceability <rule>` | Walk a rule back to the decision and requirement behind it |
| `provenance coverage scan --path .` | Match `@provenance` code annotations against rules |
| `provenance skills install` | Install the bundled agent skills (`provenance-shaping`, `provenance-fork-tournament`, `provenance-swarm-backtrace`, `provenance-grounded-writing`) |

The repository uses the `skills/<name>/SKILL.md` layout, so the bundled skills can also
be installed through the skills.sh ecosystem with `npx skills add <owner/repo>`.

### Documentation

- [Shaping](docs/shaping.md), the refinement method and how agent sessions run it
- [CLI](docs/cli.md), the full command surface
- [State format](docs/state-format.md) and [cache](docs/cache.md), how storage works

Licensed under BUSL-1.1.
