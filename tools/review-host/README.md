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

Build the SDK in this repository and build the web renderer against that exact
local SDK. The published SDK 0.2.2 does not have the protocol 8 cursor contract.
The renderer must consume the generated page entries and continuation fields.
See [the backend handoff](../../docs/cursor-reads.md).
An older whole-scope renderer cannot consume these responses.

```sh
npm ci --prefix tools/operation-codegen
node tools/operation-codegen/ensure-generated.mjs
npm ci --prefix packages/provenance
npm run build --prefix packages/provenance
npm ci --prefix tools/review-host
node tools/review-host/build.ts /absolute/web/dist-review packages/provenance /absolute/new-assets
PROVENANCE_REVIEW_ASSETS_DIR=/absolute/new-assets cargo build -p provenance-cli --bin provenance
```

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

```sh
node --test tools/review-host/session.test.ts
PROVENANCE_REVIEW_TEST_BINARY=/absolute/provenance \
  PROVENANCE_REVIEW_REAL_REPO=/absolute/provenance-repository \
  node --test tools/review-host/integration.test.ts
```

The integration test uses local Chromium from Playwright. It creates a temporary
Git repository, saves canonical graph files, and checks the actual host through
the browser. It retains its temporary repository and screenshot for inspection.
The optional second test reads `req_review_page_replaces_wiki` from the selected
existing repository without changing its canonical graph.

## Publication prerequisites

This implementation does not publish an SDK or renderer, change the release
version, or replace the existing PR 10 renderer pin. Production packaging needs:

1. A published SDK version containing the generated `readDocument` method and
   `ReadDocumentSuccessOutput` schema from this change.
2. A web dependency and lockfile update to that available version, followed by a
   validated renderer archive from the web change.
3. An exact commit, archive, and SHA-256 pin for that available renderer artifact,
   followed by this host build and CLI embedding with matching SDK declarations.

The current `tools/review-assets.json` names the older empty-shell renderer. It
does not claim to supply this application. Local verification uses the actual
source builds and their content hashes; it is not an end-to-end release.
