import type { components } from "@quality-sh/provenance/client";

type GetResponse = components["schemas"]["GetRuleSuccess"];
type Stamp = components["schemas"]["GetRuleSuccessResponseMetaStamp"];

const stamp: Stamp = {
  serial: 41,
  digest: "sha256:0000",
  instance_id: "5a1e0f1e-0000-4000-8000-000000000000",
  derivation: 0,
  policy: "catch_up",
  attested: [],
  live: ["canonical"],
};

// Trace answers always carry the page facts their producer requires.
const stamped: GetResponse = {
  data: { id: "rule_a", max_depth: 1, nodes: [] },
  meta: { stamp, limit: 50, has_more: false },
};

const degraded: GetResponse = {
  ...stamped,
  meta: {
    stamp: { ...stamp, policy: "catch_up_failed" },
    freshness_error: "catch-up failed; answer uses the stored projection",
    freshness_cause: "catch_up_failed",
    limit: 50,
    has_more: false,
  },
};

export { degraded, stamped };
