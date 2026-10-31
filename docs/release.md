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

The workflow publishes the Rust crates in
[the release crate list](../.github/release-crates.txt), in the listed order.
Preflight and the registry smoke check use the same list. Preflight requires
HTTP 200 from crates.io for each crate before a publish job can start.

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
cargo package --workspace --locked --allow-dirty --registry crates-io
```

The explicit registry selects crates.io for the package check. Keep test-only
workspace dependencies unversioned when a version would create a package
dependency cycle. Cargo omits unversioned path test dependencies from the
published manifest.

Generated client source stays outside Git and enters the crate archives. Cargo
requires `--allow-dirty` for those files. The preceding checks reject changes to
tracked source and untracked files that Git does not ignore.

- [ ] On the release commit, run the `Benchmarks` workflow with the release
  gate as the section below gives. Prepare its `repository_state` rows and
  any cause explanation for the release notes before tagging.

Run the `Benchmarks` workflow from GitHub Actions on the release branch:
set `baseline` to the previous release tag, `candidate` to the full release
commit id, and `gate` to `release`. Keep the other inputs at their defaults.

```sh
gh workflow run Benchmarks --ref <release-branch> \
  -f baseline=<previous-tag> -f candidate=<release-commit> -f gate=release
```

The job builds the release timing test of both commits and runs the two
binaries in turn, 15 times each, on one copy of the baseline's
`.provenance/state`. The job summary and the `timing-report` artifact give,
for each row, the median of each side, the median of the paired ratios
candidate/baseline, and a 95% interval for that ratio. The artifact also
holds the raw outputs. Run the workflow once, rather than repeat it to
select a faster result. If the report warns that the comparison test files
differ between the two commits, the two sides do not time the same steps.
Name that difference in the notes.

The release gate applies to the `repository_state` section: the seven
operation summary rows that it prints, and `catch_up_ms`. A row fails only
when **both** conditions hold: the lower bound of its ratio interval is
more than 2, and its candidate median is more than 5 ms above its baseline
median. A row that only the candidate prints is not a regression. A gated
row that the candidate does not print fails. A failed row fails the job and
sends the release back until the cause is named in the notes or fixed. Do
not push the version tag before this check passes or the notes name the
cause.

Record `scan_ms` and `rebuild_ms`, but do not gate them. Scan time depends on
the checkout's file count. The rebuild measurement includes preparation of
a fresh database for this store. Earlier report sections have already
opened SQLite databases. The other report sections are not gated.

To examine a result on a local host, run the timing test from a checkout
that is not inside another checkout. Cargo reads `.cargo/config.toml` from
the current directory and from all parent directories.

```bash
cargo test -p provenance-store --release -- --ignored timing_comparison_rows --nocapture
```

After the `Release` workflow generates the notes, add the `repository_state`
rows of the report and any cause explanation to the GitHub Release body under
`Timing (release build, <commit>)`, with the full release commit id and the
`Benchmarks` run link.

Tag and push the release commit:

```sh
git tag v0.3.1
git push origin v0.3.1
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
`v0.3.1-rc.1` is the way to rehearse a release without announcing one. npm
publishes that version under the `next` tag; stable versions use `latest`.

## Never `--all-features`

Release and CI builds of `provenance-cli` must enumerate features explicitly
(today: `--features scanner`). The `dogfood` feature is dev-only (internal
agent feedback capture) and must never ship in a released binary;
`--all-features` would compile it in. CI enforces this by building the
release binary with the release feature set and asserting it contains no
`dogfood` marker string.

## Build without publication

Run `gh workflow run release.yml --ref main` to build and package all release
targets at the version in the source manifests. Manual runs upload archives,
npm engine packages, and Cargo timing reports as workflow artifacts. They skip
publication and the checks of published installations. Tag pushes retain the
CI gate and the publication sequence.

A manual run on `main` can populate release build caches for later tag runs.
Caches from a tag are scoped to that tag; later tags can restore caches from
`main`, but cannot restore another tag's cache.
