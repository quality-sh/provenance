# Agent Cargo CI checks

The `scripts/agent-bin/cargo` shim sends Provenance build, check, Clippy, and test commands to a manually dispatched GitHub Actions workflow. It is opt-in. Put its directory before native Cargo on `PATH` in the process that launches agent shells:

```sh
export PATH="/path/to/provenance/scripts/agent-bin:$PATH"
```

Configure the agent host's GitHub CLI login for `quality-sh/provenance`, then run `gh auth setup-git` so Git pushes use that login. The login needs permission to push a branch and read Actions runs. Argument-bearing Cargo commands also need permission to dispatch Actions. If agents work through a long-lived OpenCode or workflowd process, set `PATH` on that process before it starts so child shells inherit it.

Plain `cargo build`, `cargo check`, `cargo clippy`, and `cargo test` calls push a snapshot branch. Its push event starts only the matching workspace-wide CI job. A call with more arguments, such as `cargo test -p provenance-cli`, uses manual workflow dispatch to run that Cargo argument list in one separate CI job from the repository root. The manual workflow must be on the target repository's default branch before argument-bearing commands can run. The shim waits and returns the selected job's status and failed-step log. Repeated calls for the same command and source snapshot reuse the run. A shell chain such as `cargo clippy && cargo test` runs both checks in CI, in order, without loading the agent host. Other Cargo subcommands execute locally through native Cargo. A remote build does not create local `target` outputs.

Set `PROVENANCE_CI_LOCAL=1` for a command that needs local compilation or build artifacts, for example `PROVENANCE_CI_LOCAL=1 cargo build`.

To build and receive a dev CLI binary without compiling locally, run `PROVENANCE_CI_ARTIFACT=1 cargo build` on a Linux x86_64 host. This is available only for plain `cargo build`. The opt-in run uploads `target/debug/provenance` for three days. After the Build job passes, the shim downloads the binary, checks that `--version` runs on this host, and installs it at the worktree's `target/debug/provenance`. It leaves an existing binary in place if the build, download, or check fails. Normal remote builds do not upload or download artifacts. If an older artifact has expired, run with `PROVENANCE_CI_RERUN=1` to create a new build.

The shim reads the current worktree into a temporary Git index and creates a deterministic snapshot commit. Plain commands push it to `agent-ci/<command>/<snapshot-sha>-<nonce>`. Argument-bearing commands push it to `agent-snapshot/<snapshot-sha>` before manual dispatch. It does not change HEAD, the current branch, or the real index. It excludes `target` and `.beads`. Stage new source files before calling a remote check; unstaged untracked files stop the run so they cannot be pushed to the public repository by accident. All included changes become public when pushed, including edits to tracked files. Git-ignored files are not part of the snapshot. Each run verifies its exact snapshot SHA after checkout.

The default destination is `quality-sh/provenance`. Set `PROVENANCE_CI_REPO` and `PROVENANCE_CI_PUSH_URL` together to target a public fork. Set `PROVENANCE_CI_RERUN=1` for one invocation to dispatch a fresh run for an unchanged snapshot. Run IDs are cached in the repository's common Git directory under `agent-cargo/`, or in `PROVENANCE_CI_STATE_DIR` when set. Share that state directory between agent processes that use the same checkout. Separate hosts can still dispatch duplicate runs for the same snapshot; workflowd can centralize this later.

The `agent-ci/` and `agent-snapshot/` refs remain on GitHub after checks complete so concurrent or delayed runs can fetch their commits. Remove old refs after their runs finish and retention is no longer needed. The shim does not currently prune them.
