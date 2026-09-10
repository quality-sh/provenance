# Release

Releases are published by GitHub Actions when a version tag is pushed.

## Targets

The release workflow builds and uploads:

- `provenance-<tag>-x86_64-pc-windows-msvc.zip`
- `provenance-<tag>-x86_64-unknown-linux-gnu.tar.gz`
- `provenance-<tag>-aarch64-apple-darwin.tar.gz`
- `SHA256SUMS`

Each native archive contains `provenance`, `cargo-provenance`, `README.md`, and
`LICENSE`. Windows uses the corresponding `.exe` names. The npm engine packages
contain only the `provenance` engine binary.

It also stages and publishes matching npm engine packages,
`@quality-sh/provenance`, and `@quality-sh/create-provenance`. npm trusted
publishing must trust `quality-sh/provenance`, the `release.yml` workflow, and
the `npm` GitHub environment for each package. The workflow uses GitHub OIDC
and does not store an npm registry token.

The workflow publishes these Rust crates to crates.io:

- `provenance-macros`
- `provenance-core`
- `provenance-scanner`
- `provenance-ste100`
- `provenance-store`
- `provenance-sdk`
- `provenance-http-client`
- `provenance-transport`
- `provenance-cli`

The `crates-io` GitHub environment protects publication. Each crate must trust
`quality-sh/provenance`, the `release.yml` workflow, and that environment on
crates.io. The official crates.io authentication action exchanges GitHub's OIDC
token for a short-lived registry token during each release. The repository does
not store a crates.io registry token.

Each engine package carries a binary and no command name. `provenance` is a
command of `@quality-sh/provenance`. The initializer adds that package as a
development dependency and then initializes the project. `npm run test:packed`
rehearses the complete flow from local archives before a release.

## Cut A Release

Update the crate and npm package versions. Verify the crate archives before you
tag the release:

```sh
git diff --exit-code HEAD --
test -z "$(git ls-files --others --exclude-standard)"
cargo package --workspace --locked --allow-dirty
```

Generated client source stays outside Git and enters the crate archives. Cargo
requires `--allow-dirty` for those files. The preceding checks reject changes to
tracked source and untracked files that Git does not ignore.

- [ ] On the release commit, run the timing report command below once. Compare
  the `repository_state` rows with the previous release's notes, meet the
  criterion below, and prepare the rows and any cause explanation for the
  release notes before tagging.

Run this command by hand in Bash, from the repository root, after all W5
release-gate changes are included. It builds and runs the ignored timing
report in release mode. It is never a continuous-integration gate, and it
needs no new Cargo task.

```bash
set -o pipefail
CARGO_BUILD_JOBS=3 cargo test -p provenance-store --release -- --ignored timing_comparison_rows --nocapture \
  | rg 'summary|_ms' > /tmp/timing-$(git rev-parse --short HEAD).txt
```

Keep the report section headed `repository_state`. Each operation's served
summary is the median across its cases; each case is the median of five
timed runs after a warm-up. The unchanged `catch_up_ms` row is also the
median of five passes. Run the command once, rather than repeat it to select
a faster result.

Compare each operation's served summary row and `catch_up_ms` with the same
row in the previous release's notes. If those notes have no comparable row,
record the missing prior row and use the current row as the reference for
the next release. A missing prior row is not a regression.
A row fails only when **both** conditions hold: it is more than twice the
previous value, and it is more than 5 ms above that value. A failed row sends the release back until the cause is
named in the notes or fixed. Do not push the version tag before this check
passes or the notes name the cause.

Record `scan_ms` and `rebuild_ms`, but do not gate them. Scan time depends on
the checkout's file count. The rebuild measurement includes preparation of
a fresh database for this store. Earlier report sections have already
opened SQLite databases.

For the first release, use these reference rows from section F.2 of the W5
release gate plan, revision 3. They were measured in a release build at
`03a935c`, before the in-place catch-up change.

| Repository state row | Reference ms | Use |
|---|---|---|
| `get` served summary (24 cases) | 1.0 | Compare |
| `search` served summary (4 cases) | 1.7 | Compare |
| Unchanged `catch_up_ms` | 21.8 | Compare |
| `scan_ms` | 0.1 | Report only |
| `rebuild_ms` | 144 | Report only |

Section F.2 gives no reference for the other operations. Record their first
served summaries as references for the next release; do not invent a prior
value. The later W5 comparison-test removal supplies timing rows for all
eight operations. That change must be included before this release check.

After the `Release` workflow generates the notes, add the `repository_state`
rows and any cause explanation to the GitHub Release body under
`Timing (release build, <commit>)`, with the full release commit id.

Tag and push the release commit:

```sh
git tag v0.2.3
git push origin v0.2.3
```

The `Release` workflow creates the GitHub Release, attaches archives, and generates release notes.

## Local Build

Build the local binary with:

```sh
cargo build --release -p provenance-cli --features scanner
```

Before a release, validate the Rust markers with
`provenance coverage scan --repo . --path crates --scope default --validate-rules`.
The command also reports active Rules that have no implementation or verification site.
Add `--strict` only when those warnings must stop the release.

The binaries land at `target/release/provenance` and
`target/release/cargo-provenance`. Users should commit `.provenance/state/` and
ignore `.provenance/cache/`.

## Versions

Every crate shares one version, set once in the workspace `[workspace.package]`
and inherited with `version.workspace = true`. The package versions in
`packages/provenance/package.json` and `packages/create-provenance/package.json`
must match it. One preflight checks the crate metadata, Cargo lockfiles, npm
manifests, npm lockfiles, and platform-engine pins before any artifact build.
The release rejects a tag unless all versions equal the tag without its `v`
prefix.

A tag with a SemVer prerelease component is published as a prerelease, so
`v0.2.3-rc.1` is the way to rehearse a release without announcing one. npm
publishes that version under the `next` tag; stable versions use `latest`.

## Never `--all-features`

Release and CI builds of `provenance-cli` must enumerate features explicitly
(today: `--features scanner`). The `dogfood` feature is dev-only (internal
agent feedback capture) and must never ship in a released binary;
`--all-features` would compile it in. CI enforces this by building the
release binary with the release feature set and asserting it contains no
`dogfood` marker string.
