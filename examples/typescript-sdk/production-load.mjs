// Loads the built application as a production install does: run it after
// `npm prune --omit=dev`, when no development dependency is present.
import assert from "node:assert/strict";

assert.throws(
  () => import.meta.resolve("@quality-sh/provenance"),
  { code: "ERR_MODULE_NOT_FOUND" },
  "a production install of the example must not contain @quality-sh/provenance",
);
const { createShareLink } = await import("./dist/share-links.js");
const now = new Date(0);
assert.ok(createShareLink(now).expiresAt > now);
