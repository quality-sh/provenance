# Repository evidence access

The default host has no repository targets. The `test-fixture` feature supplies
isolated targets and explicit target/scope grants. These tests do not authorize
real repository hosting. The [local review host](review-host.md) defines the
production caller and repository access contract.

The catalog adds `impact`, `resolve-symbol`, `evidence`, `stale`,
`verification-runs`, and `verification-bindings`. The four structured reads use
repository, scope, and optional freshness. The two verification lists accept
repository and scope, with an optional Rule filter. They reject freshness and
page controls. Native and HTTP list results remain complete arrays. MCP returns
the complete array in `{"result": [...]}` and declares that object in its output
schema. A queue refusal never returns a partial list as a complete answer.

Evidence without a base reads verification runs but does not request Git.
A head without a base retains this behavior. A base adds canonical evidence and
Git. Live stamp words retain their separate meanings. Scans do not create
canonical bindings. Each evidence collection keeps its own cut flag; the impact
scan has a separate `scan_cut` flag.

## Held source files

`operations/files` owns the open and scan seam. It returns an open regular file
and its checked relative identity. The parser consumes bytes from that handle;
it does not reopen the name. All selected source paths reject absolute paths,
parent segments, empty segments, dot segments, backslashes, drive prefixes, and
colon forms. Native authoring converts an absolute path below its trusted root
before this seam. Native Windows spelling accepts equivalent ordinary and
verbatim disk/UNC prefixes and emits slash-separated relative paths. Root
matching ignores ASCII case; other Unicode case changes refuse conservatively.
Wire inputs retain the strict portable spelling. Symlinks in selected scanned
paths are refused, including links to
another file inside the repository. Missing symbol files and unsupported source
languages retain binding-only answers. Access and containment failures refuse;
they cannot become empty successful scans. Native verification now also refuses
a missing file before it publishes a binding or run. This changes the former
acceptance of missing relative verification paths. It does not change the
binding-only result for a missing resolve-symbol file.

On Unix, each directory opens relative to a held parent with no-follow flags.
The source file opens with no-follow and nonblocking flags, and its held metadata
must identify a regular file. A metadata probe rejects known special files
before open. A FIFO replacement cannot block the open. Tree enumeration also
uses held directory handles. Each child opens relative to that directory.
The walk keeps directory frames on the heap, so deep trees do not recurse on
the thread stack. Descriptor exhaustion returns a read refusal. The walk sorts
sibling names, counts source files only, and reports an extra
source file as a cut. It excludes `.git`, `node_modules`, and `target` by name
before entry access, including excluded symlinks. It skips non-UTF-8 names as the
previous scanner did. A tree symlink without a supported source extension is
skipped without following its target, including installed skill-directory
links. Source-language symlink entries refuse. Selecting any symlink or a file
below a symlink still refuses.

On Windows, each child opens by one name relative to a held directory through
`fs_at`. The open returns the reparse object itself. Held metadata rejects every
reparse-point tag before enumeration or content reads. Attribute probes do not
request file contents. Source reads and directory enumeration use held handles;
they do not reopen an absolute path. The drive or UNC share anchor is trusted,
and each remaining root component receives the same reparse check. Other device
namespace prefixes refuse.

The required file-access CI job uses the release platform inventory: Linux GNU,
Windows MSVC, and macOS ARM64. Windows tests exercise held reads, reparse-point
refusal, replacement, and bounded enumeration on Windows. Cross-compilation
checks types and platform interfaces; it cannot establish runtime containment.

The root handle opens per operation. Configured root paths and their parents
must stay trusted between operations. Repository discovery may canonicalize
that trusted root. Source traversal below a held root never canonicalizes and
reopens a selected name. A root or ancestor rename after opening retains the
original directory identity; it does not switch to a replacement pathname.
This is a handle identity guarantee, not a promise that a file remains beneath
the root's current text pathname after a concurrent rename.

Review journal and snapshot reads also use this held-file seam. These reads
resolve only the configured repository root, so a trusted root or parent alias
is accepted. Each internal path component, including `.provenance`, must pass
the no-follow checks. Review reads use the returned regular-file handle for
metadata and content; they do not open the selected path again.

The threat model excludes hostile mount changes and the ability to create
arbitrary device nodes. A privileged device replacement during the metadata/open
interval could have device-open effects before held metadata rejects it.
Ordinary file-owner symlink and path replacements remain covered by held
traversal. Hard links identify regular files and are not rejected; the configured
repository owner must be trusted to choose its regular-file contents.

## Other trusted inputs

This change protects selected code files and the bounded and unbounded scans
used by query reads and native plans. It does not convert all storage into a
sandbox. Canonical shards, manifest, run JSONL and advisory locks, cache database,
publication and recovery paths, settings, project dictionary references, and the
machine dictionary index remain trusted control data. These paths still use
existing storage readers and writers. Their owners, ancestor directories,
symlinks, and configuration must be trusted. Target/scope grants alone do not
make malicious control-directory paths safe. The local review host requires
these control paths and their owners to be trusted.

Git executable selection, local config/includes, object storage, alternates,
worktree gitdirs, and mount topology remain trusted. The read command seam removes
inherited `GIT_*` overrides, disables filesystem monitoring and terminal prompts,
and supplies `--no-lazy-fetch` when the local git supports it (git 2.47 and
newer, detected once per process from `git --version`). Git versions that lack
the flag run without it instead of refusing; on those hosts the seam cannot
prevent an implicit fetch for a missing promised object, and the security test
that pins that property runs only where the flag exists. Diffs also disable external diff
and text conversion, and revision resolution uses `--end-of-options`. Revision
failures are typed at the command result. No code parses stderr into a domain
tag. Native diagnostics retain stderr; transport responses omit it.

The existing bounded execution admission runs host work on blocking workers.
Cancellation does not abandon started work; shutdown joins the tracked work.
Publication ownership starts and ends within the same worker invocation.

## Verification limits

Linux tests exercise selected and ancestor symlinks, portable invalid paths,
root replacement after open, repeated ancestor replacement, FIFO refusal,
ignored symlinks, deterministic cuts, real Git revisions, and a missing promised
blob whose control run invokes a remote helper. They also cover all six HTTP and
MCP operations, complete arrays, all four cuts, scope isolation, and no-base
behavior. Passing these tests does not certify every filesystem or production
host configuration. Platform runtime checks must run on their target operating
systems before release.
