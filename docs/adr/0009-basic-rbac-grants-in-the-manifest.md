# ADR 0009 (candidate): Basic RBAC grants in the manifest

Status: candidate draft, recorded by the provenance-cvs implementation bead.

## Context

The engine needs repository-local access control that survives the existing
trust model: actor IDs are attestations, there is no authentication, and state
changes only through reviewed commits or checked writes. Disposition authority
today travels in a legacy manifest allowlist (`disposition_actor_ids`) whose
empty-list-blocks-all posture is a safe default but a narrow tool.

## Decision

The manifest gains an optional `rbac` section. Each assignment names one
principal (`actor_id`), an optional `identity_type` reusing the core
`IdentityType` enum, positive capabilities from the closed set `read`, `edit`,
`execute`, `manifest-write`, and explicit `scopes`. Grants are flat and
positive-only: no wildcards, delegation, expiry, or role hierarchy. The engine
exposes no verb that writes the section; grants change only through reviewed
commits.

Enforcement is one policy function. Authorization arrives as one explicit
typed claim (`RbacClaim`) supplied top-down from the CLI (`--actor-id`) and
SDK (`actor` field), resolved against the manifest inside the publication
lock. A repository without the section behaves exactly as before; a
repository with the section refuses any mutation whose claim is missing or
unauthorized (default deny). A disposition's recorded actor must resolve to an
assignment whose `identity_type` is `human`; an assignment without
`identity_type` fails closed.

Repo-global resources (the manifest itself, the project dictionary, scope
import, re-init) demand the capability on every scope then listed, so adding a
scope narrows repo-global authority until grants cover it.

The merge driver receives its claim as a literal `--actor-id <id>` argument in
the clone-local `merge.provenance-jsonl.driver` command template, because
`.gitattributes` cannot pass arguments. The value is an attestation configured
at clone setup; the engine sniffs nothing.

The principal makes no authentication claim. `docs/adr/0001-immutable-proposal-lifecycle.md`
states that repository and CLI access is trusted; that stays true, and this
ADR adds capability structure on top of it without turning the actor ID into
an identity proof.

## Legacy window and replacement

`SDK_PROTOCOL_VERSION` moved 5 to 6 to open the window. Inside the window the
legacy `disposition_actor_ids` law applies byte-for-byte to legacy-only
manifests, and a manifest holding both a non-empty legacy list and an `rbac`
section is refused as ambiguous. At the next protocol bump, one change removes
the legacy field, the aggregate allowlist law, and the init flags, replacing
them with the human-ratification rule that already governs rbac-managed
repositories. The replacement cannot retain the empty-allowlist deadlock
because no code path consults an allowlist after the field is gone.

## Consequences

- Grant edits are Git-review-only; there is nothing to run to apply them.
- `read` ships ungated in v1; the reader layer is the later attach point.
- Between a grant change and a merge-driver config update, the stale literal
  id resolves as unauthorized and merges refuse closed.
