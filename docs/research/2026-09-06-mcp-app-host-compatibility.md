# MCP App host compatibility for the shared document view

Status: research for human review  
Date: 2026-09-06 (all sources accessed on this date)  
Related question: `question_shared_view_mcp_app_host_support` — "Can an MCP app provide the shared document view with host themes, record editing, and selected context sent to chat across the intended hosts?"  
Scope: research only. This document does not select an architecture. It changes no lifecycle behaviour.

## Question studied

Provenance is shaping a nested document view of Requirements, Rules, Resolutions, and Sources, with record-linked discussion. The view needs inline field editing, create-in-dialog, per-comment reply threads, and sending selected record or source context into agent chat. It should inherit host theme values when available, and otherwise use a Provenance theme. This report studies whether an MCP App can deliver that view across hosts, and what each host constrains.

## Executive recommendation

1. An MCP App is a viable delivery route on four hosts: **Claude Desktop, ChatGPT, VS Code, and Cursor**. Each host documents MCP Apps support as of 2026-09-06.
2. **Codex is not a documented MCP App host today.** OpenAI documents MCP for Codex as tools and server instructions only. Plan a text-only fallback for it. Do not infer Codex support from ChatGPT Apps material; OpenAI's own docs separate the two.
3. One renderer with thin host adapters is **plausible but not sufficient alone**. The MCP Apps extension defines one bridge, the official SDK auto-detects the host, and both Anthropic and OpenAI document cross-host use. But each host advertises an unknown, different subset of optional capabilities. The app must feature-detect at runtime and degrade per host.
4. Theme inheritance works through the spec's host context: a `light`/`dark` value, a standardized CSS variable set, optional fonts, and a live change notification. Hosts may pass a subset or nothing. Keep a complete Provenance theme as the fallback. User custom themes can reach the app only as variable values; per-host variable sets are mostly not published.
5. The practical blockers are transport reachability (ChatGPT needs a public HTTPS endpoint or a tunnel; Claude remote connectors run from Anthropic's cloud, not the user's machine), declared CSP connect domains, and unverified per-host support for the "send context to conversation" bridge methods.

## Surfaces this report separates

These surfaces are different products. Claims about one do not transfer to another.

- **MCP App**: HTML the MCP server returns as a `ui://` resource, which the host renders in a sandboxed iframe inside the conversation, talking to the host over the `io.modelcontextprotocol/ui` JSON-RPC bridge [1][2].
- **ChatGPT Apps vs Codex**: ChatGPT (consumer chat) renders plugin UI under the MCP Apps standard. Codex (CLI, ChatGPT desktop app "Codex" host, IDE extension) is a coding agent that consumes MCP tools. OpenAI's plugin troubleshooting page states the split: "Server, tool, and discovery checks apply to plugins in ChatGPT and Codex. UI, widget state, and client-authentication checks on this page describe ChatGPT behavior." [8]
- **VS Code extension webview vs MCP App**: a webview is arbitrary HTML that an extension controls in editor surfaces, themed through `--vscode-*` CSS variables [12]. An MCP App is UI returned by an MCP server and rendered inline in chat [4][5]. Different mechanism, different owner, different surface.
- **Ordinary browser page vs host-integrated view**: a localhost page open in a browser has no host bridge, no theme feed, no conversation context, and no proxied tool calls. It is a separate surface from any host-integrated view, even when the HTML is identical.

## Host and capability matrix

Evidence grades: **D** = documented in official primary sources (quoted below); **S** = source or community evidence only (issues, forums, changelog-adjacent); **U** = unknown, not documented either way. No cell comes from hands-on tests; no host was run for this report.

| Capability | Claude Desktop | ChatGPT | VS Code (Copilot Chat) | Cursor | Codex (CLI / desktop / IDE) |
|---|---|---|---|---|---|
| MCP Apps UI documented | D [3][4] | D [7][8] | D [5] (stable 1.109) | D [6] (2.6+) | **No** — tools only [9] |
| Local MCP servers (stdio) | D [3] | D via tunnel only [8] | D [5] | D [6] | D [9] |
| Remote servers | D (streamable HTTP; legacy SSE deprecated) [3] | D (streamable HTTP only) [8] | D (HTTP + OAuth) [5] | D (stdio, SSE, streamable HTTP) [6] | D (streamable HTTP) [9] |
| Theme values passed to app | D (light/dark + CSS variables + fonts) [3] | D (theme + standardized variables, since 2026-05-28) [8] | D mechanism / U exact values [1][5] | D mechanism / U exact values [1][6] | U (no app surface documented) |
| Live theme change notification | D [3] | D [8] | U (spec MAY) [1] | U (spec MAY) [1] | n/a |
| Custom user themes inherited | U (own palette documented; no user themes) [3] | U (light/dark only documented) [8] | U (needs test; fork themes exist) | U (needs test; fork themes exist) | n/a |
| Tool calls for writes from app UI | D (spec `tools/call`; display consent prompt) [1][3] | D (`callTool`; approval gates) [7][8] | D mechanism / U per-host UX [1][5] | D mechanism / U per-host UX [1][6] | n/a (model-initiated approvals only) [9] |
| Send message/context to conversation | D (`ui/message`, `updateModelContext`) [1][3] | D (`sendFollowUpMessage`, `setWidgetState`) [7][8] | U (needs test) | U (needs test) | No |
| Links from app | D (`ui/open-link` + confirmation modal) [3] | D (`openExternal`; vetted domains) [7][8] | U (spec-gated) [1] | U (spec-gated) [1] | No |
| Display modes | D (inline, fullscreen, pip) [1][3] | D (inline, fullscreen, pip) [7] | U (inline documented) [5] | U (inline documented) [6] | n/a |

Protocol membership does not imply parity: every capability above the first row is optional for the host under the extension spec, and hosts advertise what they support through `hostCapabilities` [1][2].

## The shared foundation: the MCP Apps extension

The MCP Apps extension is identified as `io.modelcontextprotocol/ui`. SEP-1865 (status Final) defines the proposal; the full specification lives in the `modelcontextprotocol/ext-apps` repository with a stable version dated 2026-01-26 and a draft in development [1][2]. Key documented points:

- A tool declares its UI through `_meta.ui.resourceUri` pointing at a `ui://` resource with MIME type `text/html;profile=mcp-app` [1].
- The host renders the HTML in a sandboxed iframe and must enforce a CSP built from the app's declared domains. If the app declares none, the default CSP has `connect-src 'none'` — the app can fetch nothing unless the host approves declared connect domains [1].
- Host context carries `theme: "light" | "dark"`, `styles.variables` (about 70 standardized CSS variable keys: background, text, border, ring, fonts, radii, shadows), optional `@font-face` fonts, container dimensions, safe-area insets, locale, and time zone. Hosts "can provide any subset of standardized variables, or not pass `styles` at all" [1][2].
- Live updates arrive through `ui/notifications/host-context-changed`; the host MAY send it on theme toggle or display-mode change [1].
- The app can call server tools through the host (`tools/call`, gated by a `serverTools` host capability), with tool `visibility: ["app"]` for tools hidden from the model. The spec says "Hosts can require explicit approval for UI-initiated tool calls", and `readOnlyHint: false` or missing annotations default to user confirmation [1][2].
- The app can post a message into the conversation (`ui/message`, "Host MAY request user consent") and update model-visible context (`ui/update-model-context`) [1].
- The app can open links (`ui/open-link`, gated by an `openLinks` host capability) and request display modes (`inline`, `fullscreen`, `pip`) [1].
- The official TypeScript SDK is `@modelcontextprotocol/ext-apps` (v1.7.5 on 2026-09-06). It provides the host-side `AppBridge`, the view-side `App`, and React bindings. The repository states there is "no supported host implementation in this repo (beyond the examples/basic-host example)" [2].

## Per-host evidence and limits

### Claude Desktop

Documented. Anthropic shipped interactive connectors with MCP Apps on 2026-01-26, available in Claude on web, desktop, mobile, and Cowork [3]. The getting-started page tests MCP Apps with a **local stdio server** in `claude_desktop_config.json`, so repository-backed local operation is documented [3]. Remote custom connectors instead connect "from Anthropic's cloud infrastructure, rather than from your local device" [3] — a localhost engine is not reachable through that path without exposure or a desktop-local install.

Themes are documented in depth: Claude passes `light`/`dark`, a published palette of `--color-*` variables with hex values for both modes, and Anthropic Sans fonts; updates arrive through the host-context-changed event [3]. User custom themes are not part of Claude's product, so this host can supply only its own values. Writes from the app go through proxied `tools/call`; Claude shows a display-consent prompt ("Allow" / "Always allow") for the app itself, and directory submissions must annotate tools with `readOnlyHint`/`destructiveHint`. Per-call approval UX for app-initiated writes is not explicitly documented — inferred to be annotation-driven. `ui/message` and `updateModelContext` are documented as live behaviours [3]. Links open through `ui/open-link` behind an "Open external link" confirmation modal; custom and local connectors always show the modal [3].

Limits: a new iframe mounts per tool call with "no host API to unmount earlier instances", so view state must survive remounting (Claude documents instance supersession for this) [3]. Tool results above about 150,000 characters route to a filesystem pointer instead of the app [3]. Local connectors get no stable sandbox domain [3].

### ChatGPT

Documented, under new naming. The former Apps SDK docs now serve the **Plugins** docs; UI is optional plugin UI "built on the open MCP Apps standard", with an alias table mapping `openai/outputTemplate` to `_meta.ui.resourceUri`, `window.openai.callTool` to `tools/call`, and `window.openai.sendFollowUpMessage` to `ui/message` [7][8]. Production servers must use the streamable HTTP transport at a public HTTPS endpoint; localhost development works only through a tunnel (ngrok or "Secure MCP Tunnel") [8]. Developer mode and direct connect are gated by "account and workspace policy" [8].

Theme support is documented: ChatGPT provides "standardized MCP Apps host CSS variables through `hostContext.styles.variables`" and updates them through `host-context-changed` (changelog 2026-05-28) [8]. Only a light/dark preference is documented; custom user themes and the exact variable values are not published. Writes from the widget use `callTool`; "ChatGPT does not load approval-gated arguments into widget values before approval; instead, the host delivers them through `ui/notifications/tool-input` once the user approves" [8]. User permission controls offer "always, before making changes, or only before important changes" (changelog 2026-06-12) [8]. Display modes cover inline card, carousel, fullscreen, and picture-in-picture [7][8].

Limits: no stdio; no direct localhost; the app directory path requires submission and review; mobile may present PiP as fullscreen.

### VS Code

Documented as shipped. VS Code 1.109 (release date 2026-02-04) states: "In this release VS Code has added support for MCP Apps. MCP Apps allow servers to display rich, interactive UI in the client. Apps are displayed automatically when servers return them." [5] Insiders had it from 2026-01-26 [4]. Local stdio servers come from `.vscode/mcp.json` or user configuration; remote HTTP servers with OAuth are documented; VS Code has supported the full MCP specification, including resources and sampling, since mid-2025 [5].

Limits: the release notes and MCP docs document rendering but not the app-facing details — which style variables VS Code passes, whether custom user themes flow into them, whether theme changes notify live apps, and whether the `ui/message` and `ui/update-model-context` bridge methods are wired. The webview `--vscode-*` variable mechanism proves nothing about MCP Apps; the two are separate systems [5][12]. Server push of context into the conversation is not documented; resources are user-attached through "Add Context > MCP Resources" [5]. These gaps need the probe app in the test plan. Post-ship maintenance is active (a remote-agent MCP Apps routing fix landed for milestone 1.137) [5].

### Cursor

Documented as shipped. Cursor's MCP docs list "**Apps (extension)** | Supported | Interactive UI views returned by MCP tools" and state: "Cursor supports the MCP Apps extension... MCP tools can return interactive UI along with standard tool output." [6] The release announcement dates support to Cursor 2.6 (2026-03-03). Cursor supports stdio, SSE, and streamable HTTP transports, with `mcp.json` configuration and OAuth [6].

Limits: theme exposure is undocumented. Cursor is a VS Code fork with custom themes, but its docs say nothing about passing theme values to MCP Apps; this is inference from the fork relationship, not documentation. Sampling is absent from Cursor's capability table. Forum reports from July 2026 describe version-specific MCP Apps regressions (a stopped `resources/read` in 3.9.16; unrendered UI in 3.11.19) — community evidence that rendering exists but can be flaky across releases [6]. Sending context from an app to the conversation is undocumented; users attach context through @-mentions of files, folders, terminals, chats, git diffs, and the browser [6].

### Codex

Not documented as an MCP App host. The Codex MCP page documents: STDIO servers, streamable HTTP servers, OAuth (CIMD and DCR), and server instructions. It says "The ChatGPT desktop app, Codex CLI, and IDE extension support MCP servers and share MCP configuration for the same Codex host" [9]. No page in the Codex docs mentions MCP Apps, `ui://` resources, widget rendering, themes, or a UI bridge. The community client matrix has no Codex row [10]. Codex documents tool approval modes (`auto`, `prompt`, `writes`, `approve`) — that is model-initiated tool use, not app-initiated UI.

Two cautions. First, the Plugins docs deliberately scope UI statements to ChatGPT [8], so ChatGPT Apps material must not be read as Codex support. Second, open community issues in the `openai/codex` repository (for example #36599 "App-origin tools/call bypasses model-only visibility in Codex Desktop", #29483 on a localhost iframe not rendering, #41280 on rejected `resource_link` results) suggest the ChatGPT desktop app's Codex host renders some MCP App UI, with defects [11]. This is unofficial evidence. Treat Codex Desktop as **unknown, not supported**: the view must remain fully usable there through tool calls and text.

## Can one renderer with thin host adapters work?

Plausible for the four MCP Apps hosts, with conditions:

- The spec defines one message surface for all hosts, and the official SDK implements both sides [1][2]. Anthropic documents cross-host use directly: "MCP Apps can run in both Claude and ChatGPT from a single codebase. The SDK auto-detects the host environment and uses the appropriate transport." [3] OpenAI documents the same convergence from its side with the alias layer [8].
- The thin-adapter layer is therefore mostly **capability detection and degradation**, not per-host UI code: detect `hostCapabilities` (openLinks, serverTools, message/updateModelContext advertisement), detect which style variables arrived, and fall back per feature.
- Two risks remain. First, per-host `hostCapabilities` sets are not published anywhere found; the app must discover them at runtime. Second, host bugs differ per release (documented for Cursor, reported for Codex Desktop), so the probe suite in the test plan needs re-running per host version.

## What falls back to plain HTML and text

The spec's own posture is progressive enhancement, and Cursor states it: "MCP Apps follow progressive enhancement. If a host cannot render app UI, the same tool still works through normal MCP responses." [6] OpenAI instructs server authors to "Keep the MCP tools useful without a component so ChatGPT and Codex can complete the workflow without UI" [8].

For Provenance this means every document-view operation needs a non-UI shape: tools return structured, readable summaries of records, threads, and diffs; editing, creating, and replying stay available as model-invoked tool calls with approvals; "send context to chat" becomes moot because the model already holds the tool output. The rich view (inline editing, dialogs, threads, selection send) is then an enhancement layered on top, not the only path. In an ordinary browser, the same renderer can run without the bridge as a static page — read-only unless given its own engine connection.

## Implementation implications (no architecture chosen)

- **Single renderer, spec-first.** Target the stable extension (2026-01-26) and the official SDK; treat `openai/outputTemplate`-style aliases as a compatibility layer, not a foundation [1][7][8].
- **Theme strategy.** Read `hostContext.styles.variables`; apply with the spec's `light-dark()` pattern; fall back to a complete Provenance theme when variables are missing. Do not assume custom host themes arrive — treat any variable value that appears as a bonus [1][2][3][8].
- **Writes.** Model record writes as explicit tools with `readOnlyHint: false`, marked `visibility: ["app"]` where they exist only for the UI. Expect and tolerate an approval step before arguments arrive (ChatGPT documents exactly this delay) [1][8].
- **Context send.** Use `ui/message` for "send this selection to chat" and `ui/update-model-context` for standing context; gate both behind runtime detection because VS Code and Cursor support is unverified [1].
- **State.** Persist view state outside the iframe (host widget state where offered, or re-derive from the engine) because Claude remounts the iframe per tool call [3].
- **Networking.** Declare CSP connect domains for the engine endpoint up front; the default CSP blocks all fetches [1].
- **Reachability.** Claude Desktop, VS Code, and Cursor can run a local stdio server directly; ChatGPT requires a public HTTPS endpoint or a tunnel; Claude remote connectors run cloud-side [3][5][6][8][9].

## Limits of this evidence

- **Documentation research only.** No host was installed or run. No cell in the matrix comes from a test. The test plan below exists to close that gap.
- **Access date.** All sources were fetched on 2026-09-06. Host behaviour moves fast: Cursor shipped MCP Apps in March 2026 and had rendering regressions by July 2026 [6].
- **Community sources.** The MCP client matrix is "maintained by the community" [10]. The Codex Desktop rendering evidence is open GitHub issues, not documentation [11].
- **Unknowns.** Per-host `hostCapabilities` advertisements; exact theme variable values for VS Code, Cursor, and ChatGPT; per-call approval UX on VS Code, Cursor, and Claude; `ui/message` support on VS Code and Cursor; whether user custom themes propagate anywhere except through variable values.
- **Not covered.** Enterprise policy controls per host beyond what is quoted; mobile hosts (Claude mobile documents MCP Apps; others were not studied).

## Follow-up checks: a targeted compatibility-test plan

Build one small probe app (`provenance-mcp-probe`) that serves a `ui://` page plus four tools: a read tool, a write tool with `readOnlyHint: false`, a `visibility: ["app"]` helper, and a context tool. The page renders the host context it received and buttons for each bridge call. Run the same probe per host and record pass/fail with host versions:

1. **Render**: tool result shows the iframe; record host, version, and sandbox origin.
2. **Theme**: dump `hostContext`; toggle the host theme; record whether `host-context-changed` fires and which variables and fonts arrive; repeat under a custom user theme (VS Code, Cursor).
3. **Write**: call the write tool from the page; record the approval UX and when arguments arrive.
4. **Context**: press send-message and update-context buttons; record whether the text reaches the conversation, and whether consent is requested.
5. **Links**: `ui/open-link` with an HTTPS URL; record modal/new-tab behaviour.
6. **Modes and sizing**: request fullscreen; resize; record supported modes and height caps.
7. **Transports**: stdio on Claude Desktop, VS Code, Cursor; streamable HTTP on all; tunnel for ChatGPT.
8. **Lifecycle**: trigger a second tool call; record whether state survives the remount (Claude) and whether teardown fires.

Codex: run the same probe through Codex CLI/desktop as tools only, confirm the text fallback path, and re-check the Codex docs and changelog for any MCP Apps statement before revising the matrix.

## Sources

All accessed 2026-09-06.

1. MCP Apps specification, stable 2026-01-26 and draft: https://github.com/modelcontextprotocol/ext-apps/blob/main/specification/2026-01-26/apps.mdx and `/specification/draft/apps.mdx`; SEP-1865: https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/seps/1865-mcp-apps-interactive-user-interfaces-for-mcp.md
2. `@modelcontextprotocol/ext-apps` v1.7.5, AppBridge API reference: https://github.com/modelcontextprotocol/ext-apps and https://apps.extensions.modelcontextprotocol.io/api/classes/app-bridge.AppBridge.html
3. Claude MCP Apps docs: getting started (local stdio config), https://claude.com/docs/connectors/building/mcp-apps/getting-started; theming, https://claude.com/docs/connectors/building/mcp-apps/transparent-theming; design guidelines, https://claude.com/docs/connectors/building/mcp-apps/design-guidelines; external links, https://claude.com/docs/connectors/building/mcp-apps/external-links; instance supersession and troubleshooting, https://claude.com/docs/connectors/building/mcp-apps/instance-supersession and .../troubleshooting; cross-compatibility, https://claude.com/docs/connectors/building/mcp-apps/cross-compatibility; interactive connectors for users, https://support.claude.com/en/articles/13454812-use-interactive-connectors-in-claude; custom connectors (cloud-side connection), https://support.claude.com/en/articles/11175166-getting-started-with-custom-connectors-using-remote-mcp; announcement, https://claude.com/blog/interactive-tools-in-claude
4. MCP Apps launch post (2026-01-26): https://blog.modelcontextprotocol.io/posts/2026-01-26-mcp-apps; VS Code blog (2026-01-26): https://code.visualstudio.com/blogs/2026/01/26/mcp-apps-support
5. VS Code: release notes 1.109 (2026-02-04), "Support for MCP Apps", https://code.visualstudio.com/updates/v1_109; MCP servers docs, https://code.visualstudio.com/docs/agent-customization/mcp-servers; full MCP spec support blog (2025-06-12), https://code.visualstudio.com/blogs/2025/06/12/full-mcp-spec-support; approvals, https://code.visualstudio.com/docs/agents/run/approvals
6. Cursor: MCP docs with capability table and "Apps (extension) — Supported", https://cursor.com/docs/mcp; release announcement (2026-03-03), https://forum.cursor.com/t/cursor-2-6-mcp-apps/153482; regression reports, https://forum.cursor.com/t/mcp-apps-client-stopped-issuing-resources-read-ui-widgets-dont-mount-regression-in-3-9-16/165222 and https://forum.cursor.com/t/mcp-apps-interactive-ui-not-rendered-in-cursor-3-11-19-works-in-3-10-11/165573
7. ChatGPT plugin UI docs (MCP Apps standard, alias table): https://developers.openai.com/plugins/build/chatgpt-ui and https://developers.openai.com/plugins/reference
8. ChatGPT plugins: MCP server concepts (streamable HTTP), https://developers.openai.com/plugins/concepts/mcp-server; connect and test (developer mode, tunnel), https://developers.openai.com/plugins/deploy/connect-chatgpt; troubleshooting (ChatGPT-vs-Codex scoping), https://developers.openai.com/plugins/deploy/troubleshooting; changelog (2026-05-28 theme variables; 2026-06-12 permission controls), https://developers.openai.com/plugins/changelog
9. Codex MCP docs (tools, transports, approvals; no UI): https://developers.openai.com/codex/extend/mcp
10. MCP extension support matrix (community-maintained; no Codex row): https://github.com/modelcontextprotocol/modelcontextprotocol/blob/main/docs/extensions/client-matrix.mdx
11. Codex community issues on MCP App behaviour (unofficial evidence): https://github.com/openai/codex/issues/36599, /29483, /30560, /41280
12. VS Code webview API (contrast only): https://code.visualstudio.com/api/extension-guides/webview
