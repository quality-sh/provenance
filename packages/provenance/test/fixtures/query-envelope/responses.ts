import type { GetResponse, Stamp } from "@quality-sh/provenance";

// An answer recorded before the stamp existed still satisfies the envelope.
const recorded: GetResponse = {
  protocol_version: 6,
  operation: "get",
  found: false,
};

const stamp: Stamp = {
  serial: 41,
  digest: "sha256:0000",
  instance_id: "5a1e0f1e-0000-4000-8000-000000000000",
  derivation: 0,
  policy: "catch_up",
  attested: [],
  live: ["canonical"],
};

const stamped: GetResponse = {
  ...recorded,
  stamp,
};

const degraded: GetResponse = {
  ...recorded,
  stamp: { ...stamp, policy: "catch_up_failed" },
  freshness_error: "catch-up refused",
};

export { degraded, recorded, stamped };
