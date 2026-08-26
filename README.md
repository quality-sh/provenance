# Provenance

Never lose the *why* behind your decisions.

Provenance is a tool for building requirements traceability, from source to requirement to rule. A Rule is an atomic behavioural obligation that can exist before its implementation; `#[rule("rule_id")]` binds production code to it, and `#[verifies("rule_id", method)]` binds evidence.

### Installation

Install and initialize Provenance in a TypeScript project:

```sh
npx --yes @quality-sh/create-provenance
```

The initializer detects npm, pnpm, Yarn, Bun, Deno, or Nub. It installs
`@quality-sh/provenance` as an exact development dependency, creates the
default scope, validates the state, and ignores `.provenance/cache/`.

Install the CLI from crates.io:

```sh
cargo install provenance-cli
```

To build the CLI from this source tree:

```sh
cargo build --release -p provenance-cli --all-features
```

The binary lands at `target/release/provenance`. Put it on your PATH.

### Quick start

```sh
# install the development dependency and set up the repository
npx --yes @quality-sh/create-provenance

# put something in the graph
npx provenance requirements create --scope default --id req_exports \
  --statement "Exports finish in under a minute"

# see where things stand
npx provenance prime
```

### Essential commands

| Command | What it does |
| --- | --- |
| `provenance prime` | Bounded low-res graph frontier; proposals surface separately when evidence or claimed territory demands them |
| `provenance check` | Validate the state files |
| `provenance materialize` | Rebuild the SQLite query cache |
| `provenance graph <requirement>` | Show the neighbourhood of a requirement |
| `provenance graph-reference issue\|show\|verify\|exact-export` | Hand off an immutable pinned graph |
| `provenance traceability <rule>` | Walk a Rule back to its Requirement and any producing Resolution |
| `provenance proposals surface --scope default --changed-path <path>` | Surface undisposed proposals when current work touches their evidence or explicit territory |
| `provenance wiki build` / `provenance wiki serve` | Build or serve the generated wiki with domain browsing and offline search |
| `provenance coverage scan --path . --validate-rules` | Check bindings and report active Rules with no implementation or verification |
| `provenance sdk check-statement` / `provenance sdk apply` / `provenance sdk verification-runs` | ASD-STE100 statement preflight, typed desired state, and callback evidence protocol |
| `provenance stale --since main` | Report whether a diff touched, moved, or removed any graph evidence path |
| `provenance skills install` | Install the bundled agent skills (`provenance-shaping`, `provenance-fork-tournament`, `provenance-swarm-backtrace`, `provenance-grounded-writing`) |

The repository uses the `skills/<name>/SKILL.md` layout, so the bundled skills can also
be installed through the skills.sh ecosystem with `npx skills add <owner/repo>`.

### Documentation

- [Shaping](docs/shaping.md), the refinement method and how agent sessions run it
- [CLI](docs/cli.md), the full command surface
- [State format](docs/state-format.md) and [cache](docs/cache.md), how storage works
- [TypeScript SDK POC](docs/typescript-sdk-poc.md), typed declarations, Node verification, and findings

Licensed under BUSL-1.1.
