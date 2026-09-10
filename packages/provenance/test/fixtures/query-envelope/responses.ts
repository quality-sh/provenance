import type { GetResponse, Stamp } from "@quality-sh/provenance";

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
  protocol_version: 8,
  operation: "get",
  found: false,
  stamp,
};

const degraded: GetResponse = {
  ...stamped,
  stamp: { ...stamp, policy: "catch_up_failed" },
  freshness_error: "catch-up failed; answer uses the stored projection",
  freshness_cause: "catch_up_failed",
};

export { degraded, stamped };
