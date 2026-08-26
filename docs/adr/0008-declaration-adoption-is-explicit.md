# ADR 0008: Declaration adoption is explicit and exact

## Status

Accepted.

## Context

The NoScope migration found 70 active Requirements with stable IDs. The
records had no Declaration owner or Declaration address. Typed authoring could
either create new IDs or request the existing IDs. The second choice caused
ownership conflicts because an unowned record was not owned by the spec.

That refusal is a safety control. An implicit match by ID, key, or document
scope could let one spec take records that another process manages. A global
adoption switch could take every unowned record in a document. A full-scope
import could change graph state that is not part of the typed declaration.

## Decision

A typed desired-state document can contain an exact `adopt_unowned` allowlist.
Each target names one resource kind and one Stable ID. The same document must
contain exactly one declaration of that kind with the same explicit ID. The
canonical record must exist.

Adoption applies only when the canonical record has no Declaration owner. The
fields supplied by the typed declaration and its relevant
Source-to-Requirement or Requirement-to-Rule relationships must be equal to
canonical state. Canonical metadata outside the typed declaration surface, or
optional metadata the declaration omits, is preserved and does not block
adoption. Adoption changes only `declared_by` and `declaration_address`. It does
not transfer a record from one owner to another.

Plan and apply use one Rust ownership decision. Plan reports a valid adoption
with the existing Stable ID and only the owner and address changes. Apply
refuses the complete document if one target is invalid. A repeated exact
request is unchanged.

The Rust and TypeScript authoring interfaces use an adoption method that also
requires the explicit ID. The wire change increments the SDK protocol from 4
to 5. The canonical state schema stays at version 1.

## Consequences

A migration has two steps. First, it plans and applies exact adoption. Then it
removes the adoption request and uses the ordinary declaration with `.id(...)`
to make later definition changes.

A typo cannot create a new record. A changed statement, supplied description,
Source field, or relationship causes a conflict and no write. A record owned
by another declaration also causes a conflict and no write. Documents with no
adoption targets keep the previous ownership refusal.
