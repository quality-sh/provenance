# Release

Releases are published by GitHub Actions when a version tag is pushed.

## Targets

The release workflow builds and uploads:

- `provenance-<tag>-x86_64-pc-windows-msvc.zip`
- `provenance-<tag>-x86_64-unknown-linux-gnu.tar.gz`
- `provenance-<tag>-x86_64-apple-darwin.tar.gz`
- `provenance-<tag>-aarch64-apple-darwin.tar.gz`
- `SHA256SUMS`

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
cargo package --workspace --locked
```

Tag and push the release commit:

```sh
git tag v0.2.1
git push origin v0.2.1
```

The `Release` workflow creates the GitHub Release, attaches archives, and generates release notes.

## Local Build

Build the local binary with:

```sh
cargo build --release -p provenance-cli --all-features
```

Before a release, validate the Rust markers with
`provenance coverage scan --repo . --path crates --scope default --validate-rules`.
The command also reports active Rules that have no implementation or verification site.
Add `--strict` only when those warnings must stop the release.

The binary lands at `target/release/provenance`. Users should commit `.provenance/state/` and ignore `.provenance/cache/`.

## Versions

Every crate shares one version, set once in the workspace `[workspace.package]`
and inherited with `version.workspace = true`. The package versions in
`packages/provenance/package.json` and `packages/create-provenance/package.json`
must match it. The release job rejects a tag unless all versions equal the tag
without its `v` prefix.

A tag carrying a hyphen is published as a prerelease, so `v0.2.1-rc.1` is the
way to rehearse a release without announcing one. npm publishes that version
under the `next` tag; stable versions use `latest`.
