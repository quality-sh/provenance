# Local review application

This directory supplies the application that the Provenance CLI embeds. The web
repository supplies `mountReview`, `loadReviewStore`, their emitted declarations,
and renderer assets. This application owns repository access, the credential
form, explicit Requirement selection, loading, refresh, and error display.

The access token stays in page memory. It is sent only in an Authorization
header to the current origin. The token input is cleared on every connection
attempt. The application does not store the token or put it in a URL. The host's
existing origin, repository, scope, and file-access checks remain in effect.
Mutation controls are unavailable. Adapter write methods throw a read-only
error, and no review acceptance is inferred from lifecycle state.

The renderer pin uses the successful main build from web PR 11 and SDK 0.2.3.
Build the SDK in this repository to check that the host contract still matches
the renderer. The composer checks the archive SHA-256, source commit, clean
build state, and generated SDK schema before it creates the output.
See [the read contract](../../docs/cursor-reads.md).

```sh
npm ci --prefix tools/operation-codegen
node tools/operation-codegen/ensure-generated.mjs
npm ci --prefix packages/provenance
npm run build --prefix packages/provenance
npm ci --prefix tools/review-host
node tools/review-host/prepare.ts packages/provenance crates/provenance-cli/review-assets-generated
cargo build -p provenance-cli --bin provenance
```

Set `PROVENANCE_REVIEW_ARCHIVE` to a saved copy of the pinned archive for an
offline renderer input. With no saved copy, the preparation script uses the
pin's HTTPS `url`, when present, or authenticated `gh run download` for the
pinned run. The latter checks that the run passed on the pinned commit.
A local SDK build and the published SDK 0.2.3 have the same schema for this pin.

For an unpinned local renderer build, use `build.ts WEB_ASSETS SDK_PACKAGE
NEW_OUTPUT`. Set `PROVENANCE_REVIEW_ASSETS_DIR` to that output when building
the CLI. This override takes precedence over the generated package assets.

The output directory must not exist. The builder checks the host's TypeScript
and verifies that the renderer and host used the same generated SDK schema.
`host-build-info.json` records the renderer source commit, dirty state, input
file hashes, SDK schema hash, and hashes of all served files. The builder keeps
the renderer module unchanged and adds `host.js`, `host.css`, and the host HTML.
The SDK HTTP runtime is bundled into `host.js`. No Node runtime enters the page.

Run the host with explicit repository and scope options from `docs/review-host.md`.
Open the printed credential-free URL. Enter the session token, then enter a
Requirement ID. `Open / Refresh` reads the saved working-copy graph each time.
A failed refresh clears the old document. A later request supersedes an earlier
request, even when the earlier response arrives last.
The session disposes a store that finishes loading after a later refresh.
Renderer cleanup disposes each mounted store when the session removes its view.

Each refresh captures the selected Requirement and the authorized repository and
scope. Each document loader fixes its root and page limit at 50. The first read
uses `catch_up`; continuation uses `annotate_only`. Opening another Requirement
creates a loader for that root. Shared search uses the same repository and scope,
page limit, and freshness policy. The renderer shows read status, including
partial pages and refusals. A mounted view does not establish completeness.

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

The upstream Actions artifact is in a private repository and expires on
2026-12-09. Authenticated retrieval is sufficient for local validation. Backend
CI and repeatable future packaging need a durable download URL for the exact
pinned bytes. A renderer URL must be supplied before this packaging path can
run with the backend workflow token. The pin does not imply publication.
