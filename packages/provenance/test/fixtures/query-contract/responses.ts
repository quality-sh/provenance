import type {
  EvidenceResponse, GetResponse, GraphNode, QueryEnvelope, SourceKind, Stamp,
} from "../../../src/protocol.js";

const stamp: Stamp = {
  serial: 1, digest: "digest", instance_id: "instance", derivation: 0,
  policy: "catch_up", attested: [], live: ["canonical"],
};

// Current responses use the v2 envelope.
// @ts-expect-error A response without metadata is not a current response.
const missingStamp: GetResponse = { data: { id: "rule_a" } };
// @ts-expect-error A flattened result cannot satisfy the v2 get contract.
const wrongOperation: GetResponse = { id: "rule_a", max_depth: 1, nodes: [] };

const missingCuts: EvidenceResponse = {
  // @ts-expect-error The current evidence result always includes all four cut flags.
  data: { rule_id: "rule_a",
    implementation_bindings: [], verification_bindings: [], verification_runs: [],
    latest_verification_run: null, review_required: false, reviews: [], stale: null },
  meta: { stamp },
};

// Generated record variants expose the tag and narrow to their real fields.
export function sourceKind(node: GraphNode): SourceKind | undefined {
  if (node.node_type === "source") return node.source_type;
  return undefined;
}

export function revision(answer: QueryEnvelope): number | undefined {
  return answer.meta.stamp?.serial;
}

export { missingCuts, missingStamp, wrongOperation };
