# Local host discovery

A local HTTP host can publish one credential-free discovery record for one
canonical repository and scope. Review links use this protocol. Other local
clients can use it when their access policy permits discovery.

The host layer owns publication, identity, endpoint validation, liveness
checks, and stale-record removal. A client calls the host-layer discovery
interface. A client does not read the registry file.

## Registry contract

The registry path is `.provenance/cache/local-hosts/SCOPE.json` below the
canonical repository root. The file contains one JSON object:

```json
{
  "schemaVersion": 1,
  "endpoint": "http://127.0.0.1:43123",
  "repositoryId": "local",
  "scope": "default",
  "instanceNonce": "0123456789abcdef0123456789abcdef"
}
```

The [registry fixture](fixtures/local-host/registry-v1.json) and the
[identity fixture](fixtures/local-host/identity-v1.json) are conformance inputs
for implementations in other languages. A version 1 implementation accepts
the field names and value types in these fixtures. It rejects a different
schema version.

The registry contains no bearer credential. `repositoryId` is an opaque target
name, not a file-system path. `instanceNonce` identifies one host process and
prevents an older process from removing a newer record.

The registry directory, file, and lock file permit access only to the operating
system owner on platforms that support these permissions. Publication uses an
atomic file replacement. The host publishes only after it binds the listener
and validates repository access.

## Endpoint and identity checks

The endpoint is an exact HTTP base URL. It has a literal loopback IPv4 or IPv6
address, an explicit nonzero port, and no user information, path, query, or
fragment. The protocol does not accept `localhost`.

The public identity route is `/local-host-identity`. Its response contains
`schemaVersion`, `repositoryId`, `scope`, and `instanceNonce`. It does not
contain the endpoint, repository path, or bearer credential.

Discovery uses short connection, read, and write timeouts. It does not follow
redirects. It returns the endpoint only when the public identity equals the
registry identity.

## Ownership and lifecycle

One discoverable host can own a repository-and-scope slot. Publication locks
the slot and probes an existing record. Publication refuses when the existing
owner is live. It replaces the record when the existing owner is stale.

Normal shutdown removes the record only when its nonce still matches the
registration. Discovery returns no host for invalid or stale state and removes
a stale record while it holds the registry lock. An explicitly addressed
listener can run without publishing a discovery record.
