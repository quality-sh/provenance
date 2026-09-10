import type {
  EvidenceResponse, GetResponse, GraphNode, QueryEnvelope, SourceKind, Stamp,
} from "../../../src/protocol.js";

const stamp: Stamp = {
  serial: 1, digest: "digest", instance_id: "instance", derivation: 0,
  policy: "catch_up", attested: [], live: ["canonical"],
};

// Current responses always carry a stamp and their exact operation identity.
// @ts-expect-error A historical unstamped response is not a current response.
const missingStamp: GetResponse = { protocol_version: 8, operation: "get", found: false };
// @ts-expect-error A search result cannot satisfy the get contract.
const wrongOperation: GetResponse = { protocol_version: 8, operation: "search", found: false, stamp };

// @ts-expect-error The current evidence result always includes all four cut flags.
const missingCuts: EvidenceResponse = {
  protocol_version: 8, operation: "evidence", stamp, rule_id: "rule_a",
  limit: 200, has_more: false, implementation_bindings: [], verification_bindings: [],
  verification_runs: [], review_required: false, reviews: [], stale: null,
};

// Generated record variants expose the tag and narrow to their real fields.
export function sourceKind(node: GraphNode): SourceKind | undefined {
  if (node.node_type === "source") return node.source_type;
  return undefined;
}

export function revision(answer: QueryEnvelope): number {
  return answer.stamp.serial;
}

export { missingCuts, missingStamp, wrongOperation };
