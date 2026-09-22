import type { GetSourceTraceInput } from "../../../src/client.js";

// The generated input types close every declared parameter selection. The
// Effect client methods take these same `*Input` types, so one rejection
// covers both surfaces.

// A valid direction typechecks and keeps its wire tokens.
export const valid: GetSourceTraceInput = {
  id: "source_a", query: "trace", direction: "in",
};

export const invented: GetSourceTraceInput = {
  id: "source_a", query: "trace",
  // @ts-expect-error an invented direction is not a published value
  direction: "sideways",
};

export const inventedSelector: GetSourceTraceInput = {
  id: "source_a",
  // @ts-expect-error an invented selector is not a published value
  query: "browse",
};
