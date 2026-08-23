# Release

Releases are published by GitHub Actions when a version tag is pushed.

## Targets

The release workflow builds and uploads:

- `provenance-<tag>-x86_64-pc-windows-msvc.zip`
- `provenance-<tag>-x86_64-unknown-linux-gnu.tar.gz`
- `provenance-<tag>-x86_64-apple-darwin.tar.gz`
- `provenance-<tag>-aarch64-apple-darwin.tar.gz`
- `SHA256SUMS`

It also stages and publishes matching npm engine packages plus
`@quality-sh/provenance`. npm trusted publishing must be configured for the
repository's `npm` GitHub environment before cutting the first package release.

Each engine package carries a binary and no command name. `provenance` is a
command of `@quality-sh/provenance`, the one package the quick start installs,
so `npx provenance` resolves the same way on every host. `npm run test:packed`
rehearses that whole install from local archives before a release.

## Cut A Release

Update crate versions, then tag and push:

```sh
git tag v0.1.0
git push origin v0.1.0
```

The `Release` workflow creates the GitHub Release, attaches archives, and generates release notes.

## Local Build

Build the local binary with:

```sh
cargo build --release -p provenance-cli --all-features
```

A release scans clean: `provenance coverage scan --path . --scope default --validate-rules --strict` exits zero, so every marker cites a real rule and every active rule has a verification site.

The binary lands at `target/release/provenance`. Users should commit `.provenance/state/` and ignore `.provenance/cache/`.

## Versions

Every crate shares one version, set once in the workspace `[workspace.package]`
and inherited with `version.workspace = true`. The TypeScript SDK version in
`packages/provenance/package.json` must match it. The release job rejects a tag
unless both versions equal the tag without its `v` prefix.

A tag carrying a hyphen is published as a prerelease, so `v0.1.0-rc.1` is the
way to rehearse a release without announcing one. npm publishes that version
under the `next` tag; stable versions use `latest`.
