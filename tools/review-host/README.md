# Local review application

This directory supplies the application that the Provenance CLI embeds. The web
repository supplies `mountReview`, its emitted declarations, and renderer
assets. This application owns the credential form, explicit Requirement
selection, configuration read, and connection error display.

The access token stays in page memory. It is sent only in an Authorization
header to the current origin. The token input is cleared on every connection
attempt. The application does not store the token or put it in a URL. The host's
existing origin, repository, scope, and file-access checks remain in effect.
The renderer owns the generated Effect client and its reads and writes.

The renderer pin uses the successful main build from web PR 23 and SDK 0.2.3.
The composer checks the archive SHA-256, source commit, and clean build state
before it creates the output.

```sh
npm ci --prefix tools/operation-codegen
node tools/operation-codegen/ensure-generated.mjs
npm ci --prefix packages/provenance
npm run build --prefix packages/provenance
npm ci --prefix tools/review-host
node tools/review-host/prepare.ts crates/provenance-cli/review-assets-generated
cargo build -p provenance-cli --bin provenance
```

Set `PROVENANCE_REVIEW_ARCHIVE` to a saved copy of the pinned archive for an
offline renderer input. With no saved copy, the preparation script downloads
the pin's public HTTPS `url` without credentials and checks its SHA-256.

For an unpinned local renderer build, use `build.ts WEB_ASSETS NEW_OUTPUT`.
Set `PROVENANCE_REVIEW_ASSETS_DIR` to that output when building
the CLI. This override takes precedence over the generated package assets.

The output directory must not exist. The builder checks the host's TypeScript
against the renderer declarations.
`host-build-info.json` records the renderer source commit, dirty state, input
file hashes, and hashes of all served files. The builder keeps
the renderer module unchanged and adds `host.js`, `host.css`, and the host HTML.
The generated Effect client is part of `review.js`. No Node runtime enters the page.

Run the host with explicit repository and scope options from `docs/review-host.md`.
Open the printed credential-free URL. Enter the session token, then enter a
Requirement ID. `Open / Refresh` mounts the selected Requirement. A new
connection removes the old page. A later connection attempt supersedes an
earlier attempt, even when the earlier response arrives last.

```sh
node --test tools/review-host/session.test.ts
```

This repository tests the host, authorization, assets, and API in Rust, and
session behavior without a browser. `provenance-web` owns Storybook component
tests and its one small browser smoke test of built assets.

## Packaging

CI composes the application once and supplies the same output to each native
build. Release builds use this path for both tag and build-only manual runs.
The Rust package includes `review-assets-generated`, so a Cargo install can
embed the application without downloading it at build time. Git ignores these
files, and the source-control check rejects them if they are staged.
A source checkout without generated assets uses the committed unavailable page.
Release preparation fails if it cannot obtain or validate the pinned archive.

The native check copies each executable outside the checkout, removes Node
from its search path, and checks every served file against the composition
receipt. It also uses the SDK for document continuations, search, and refused
repository and scope access. Release validation runs this check on executables
extracted from both native archives and npm engine packages. It does not
publish a release during a manual run.

```sh
node --test tools/review-host/*.test.ts
node tools/review-host/verify-native.ts /absolute/provenance crates/provenance-cli/review-assets-generated packages/provenance
```

The [public renderer archive](https://github.com/quality-sh/provenance/releases/download/renderer-f9e60a1c9523/provenance-review.tar.gz)
contains the exact bytes from the pinned upstream build. The release contains
only the renderer. It is separate from the CLI releases and is not the latest
CLI release. CI and release preparation use this archive without access to the
private web repository.
