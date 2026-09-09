# Local review host

Run `provenance review --repo /absolute/repository --repository-id A --scope default`.
The repository must contain an initialized Provenance manifest and the selected
scope. All three options are required. `--port 0` selects an available port;
a specified port binds only to `127.0.0.1`.

The CLI owns the listener. Ctrl-C or SIGTERM stops admission, waits for started
operations, and closes the listener. After operations finish, HTTP connections
have one second to drain. Incomplete headers or bodies cannot hold the process
open. Closing stdin does not stop this interactive
host. Native CLI and Rust calls still use their direct operation paths.

## Local caller and repository access

Startup writes one JSON line to stdout with `endpoint`, `bearer`,
`repositoryId`, `scope`, and `url`. The URL is `http://127.0.0.1:PORT/`; it contains
no credential. Treat the JSON output as a credential. Each process generates a
new token. The token is not saved in the repository, a cookie, browser storage,
a URL query, or a URL fragment.

The local caller is the person who starts the process and the clients to which
that person gives its token. This is one local principal, not a user account
system. Starting the host grants that principal access to the existing catalog
operations for the configured repository and scope. Native validation, file
access checks, publication locks, and disposition actor grants still apply.
Record actor and role fields do not authenticate a caller.

The selected repository, its control files, Git configuration, and ancestor
directories must be under the local caller's control. The host uses the existing
[repository file access](operation-file-access.md) contract. It does not sandbox
hostile repositories or isolate other processes running as the same OS user.

Every operation and configuration request requires `Authorization: Bearer TOKEN`.
The host checks this credential before it decodes an operation body. All routes
require the bound Host value. Requests with an Origin must name the exact local
origin. `Origin: null`, unrelated origins, and cross-site browser requests are
refused. Requests without Origin remain available to authorized non-browser
clients. The host does not enable CORS. Cookies and URL parameters do not grant
access. The exact Host check also refuses DNS rebinding through an attacker
hostname, even if that hostname resolves to loopback. Duplicate security headers
refuse. Assets and protocol metadata contain no graph data and require no bearer
token; they retain the same destination and origin checks.

Repository IDs are opaque names, not paths. The host resolves only its configured
ID and refuses all other IDs. It refuses a different scope before it reads
settings or prepares storage. Change repository or scope by restarting the host.

## Browser integration contract

Build the browser assets before building the Rust binary. Set
`PROVENANCE_REVIEW_ASSETS_DIR` to an asset directory with `index.html` at its root
and all local dependencies below that root. The Cargo build embeds those files
in the binary. No Node process or asset directory is needed at runtime.
The directory is a build input; generated asset files must not be committed to
this repository. The host rejects symbolic links and reserved route names in
the asset tree at build time. This directory is trusted build input: replacement
JavaScript would run with the browser caller's access. Use the pinned archive
procedure below for the supplied renderer.
Path segments use ASCII letters, digits, dots, underscores, and hyphens. A
segment must not start with a dot. Root names `metadata`, `review-config`, and
`v` followed by digits are reserved for host routes.

Without that build input, the binary embeds
`crates/provenance-cli/review-assets/index.html`. This page states that the review
bundle is unavailable. It contains no review records, fixture data, or simulated
mutations. This fallback is not the review UI. Builds that need the supplied
renderer must set the asset input explicitly. This change does not change all
release jobs to fetch the renderer automatically.
The CLI's production dependency `provenance-transport` is included in the Cargo
release order before the CLI. It uses the shared workspace release version.

The asset entry is `/index.html`, also served at `/`. Relative local dependencies
retain their paths. Missing assets return 404; no repository file is served.
`/metadata` and `/v7/operations/:operation` retain their existing contracts.
The host reserves `/review-config` for authenticated runtime configuration:

```json
{"endpoint":"http://127.0.0.1:PORT","repositoryId":"A","scope":"default","protocolVersion":7,"sdkVersion":"0.2.2"}
```

The merged renderer does not read credentials, fetch configuration, load an SDK,
or mount itself. Its `index.html` is an empty shell with a stylesheet and a root
element. The consumer owns credential entry and transport, loading and error
handling, the concrete SDK-to-view adapter, and this call:

```js
import { mountReview } from './review.js';
const unmount = mountReview(element, { store, initialSelectedId });
```

`store` must satisfy the renderer's `ReviewStore` interface. The return value
unmounts the view. The document ticket (`provenance-boc5.3`) owns that bootstrap,
the complete document adapter, and real read integration. It must keep the
credential in memory, send it only in the Authorization header to the exact
local endpoint, and prevent it from entering URLs, logs, or browser storage.
If a later launcher uses a fragment for credential delivery, its consumer must
remove that fragment immediately. This host does not issue such a URL.

The browser entry is `@quality-sh/provenance/client`, exact SDK version `0.2.2`,
operation protocol `7`. The generated client calls an existing host; it does not
start one. Asset code must use the same origin and local dependencies. Response
policy permits local scripts, styles, fonts, images, and connections. It blocks
framing, inline scripts, external connections, and service workers.

This host can serve assets and existing reads before additional review writes
exist. It adds no approval mapping or review operation. Approval into the system
description and lifecycle changes remain separate. Discussion `created_at`
values remain logical counters. Declaration `apply` can retire omitted owned
declarations; it is not a field patch. Wiki removal remains a separate ticket;
this host has no dependency on provenance-boc5.6.

## Pinned renderer input

The pin in [`tools/review-assets.json`](../tools/review-assets.json) identifies:

- Source repository: `quality-sh/provenance-web`.
- Merged commit: `93678e4a3a3308b01300c57cc613788af4d54def` (PR 10).
- Validated author commit: `3efad03e2e2d5c46eea4b14043d674b62d1c1d63`.
- Successful [Review assets run 34346002187](https://github.com/quality-sh/provenance-web/actions/runs/34346002187).
- Archive: `provenance-review.tar.gz`.
- SHA-256: `138da3a6722348ec8d382304509c0822b38da46a8afd57f050af0d5263345207`.

The upstream entry is `src/browser/main.tsx`; `vite.review.config.ts` builds
`dist-review`. `scripts/package-review.mjs` archives that directory with stable
metadata and emits a checksum sidecar. The archive includes `review.js`,
`review.css`, local fonts under `assets/`, the empty `index.html`,
`build-info.json`, and `licenses/provenance.txt`. Its renderer does not contain
an SDK runtime or a configured application. See the merged
[browser contract](https://github.com/quality-sh/provenance-web/blob/93678e4a3a3308b01300c57cc613788af4d54def/src/browser/README.md)
and [SDK contract](https://github.com/quality-sh/provenance-web/blob/93678e4a3a3308b01300c57cc613788af4d54def/src/review/sdk-contract.md).

Prepare a new directory outside source control, then build:

```sh
PROVENANCE_REVIEW_ASSETS_OUTPUT=/tmp/provenance-review-assets \
  cargo test --locked -p provenance-cli --test review_bundle -- --ignored --nocapture
PROVENANCE_REVIEW_ASSETS_DIR=/tmp/provenance-review-assets cargo build --locked -p provenance-cli --bin provenance
```

Preparation and validation are one ignored Rust test. It uses authenticated
`gh run download` for the pinned workflow artifact. It verifies the archive
against the committed SHA-256 before it creates any output directory. Set
`PROVENANCE_REVIEW_ARCHIVE=/path/to/provenance-review.tar.gz` to use a saved
copy without GitHub access. Set `PROVENANCE_REVIEW_BINARY_OUTPUT` to retain
the validated standalone binary. A sidecar alone cannot change the accepted
hash. GitHub workflow artifacts can expire; retain the matching archive for
reproducible builds. An unavailable download or changed checksum stops
preparation. No network access occurs in the Cargo asset build step or at runtime.

`.2` supplies the production local host, authorization, explicit repository and
scope access, embedded static serving, shutdown, and this verified build-input
contract. Serving the generic shell does not demonstrate a connected review
experience. `.3` owns the remaining document integration. `.5` owns persistent
review mutations; `.6` owns Wiki removal. SDK/client package separation remains
`provenance-cftl` and is outside this change.

## Validation

Run `cargo test -p provenance-cli --test cli_review_host --test review_asset_build --test review_archive`
and `cargo test -p provenance-transport --features test-fixture` after generation.
Run `cargo test -p provenance-cli --test review_bundle -- --ignored` to build with the
pinned real archive, or set `PROVENANCE_REVIEW_ARCHIVE` to the saved copy.

The bundle check compares every served file with the verified archive. It copies
the executable, deletes the build input, removes Node from its process search
path, and checks authenticated existing reads, target refusal, and listener
closure. It does not mount a fixture store or claim complete document integration.
The host tests submit valid denied reads and writes and compare repository bytes
and directory entries. They also test foreign origins, DNS-rebinding Host values,
duplicate headers, query/cookie credential refusal, held source-file traversal,
invalid startup configuration, and shutdown with incomplete HTTP requests.

Linux validation does not establish Windows or macOS signal and file-access
behavior. Existing platform CI remains responsible for those checks. Native
operation tests continue to use direct calls without a host credential.
