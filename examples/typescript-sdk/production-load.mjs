// Installs the example as a deployment does, with `npm ci --omit=dev` in a copy
// of its package files and built output, and loads the built application from
// that copy.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { cpSync, mkdtempSync, rmSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const example = dirname(fileURLToPath(import.meta.url));
// A directory beside the example keeps the relative `file:` paths of the lock
// file true in the copy.
const install = mkdtempSync(join(dirname(example), "typescript-sdk-production-"));
try {
  for (const entry of ["package.json", "package-lock.json", "dist"]) {
    cpSync(join(example, entry), join(install, entry), { recursive: true });
  }
  const ci = spawnSync(
    "npm",
    ["ci", "--omit=dev", "--ignore-scripts", "--no-audit", "--no-fund"],
    { cwd: install, encoding: "utf8" },
  );
  assert.equal(ci.status, 0, ci.stderr);

  const resolve = spawnSync(
    process.execPath,
    ["--input-type=module", "-e", 'import.meta.resolve("@quality-sh/provenance");'],
    { cwd: install, encoding: "utf8" },
  );
  assert.notEqual(resolve.status, 0, "@quality-sh/provenance resolves in a production install");
  assert.match(resolve.stderr, /ERR_MODULE_NOT_FOUND/);

  const { createShareLink } = await import(
    pathToFileURL(join(install, "dist", "share-links.js")).href
  );
  assert.deepEqual(createShareLink(new Date(0)), {
    expiresAt: new Date(7 * 24 * 60 * 60 * 1000),
  });
} finally {
  rmSync(install, { recursive: true, force: true });
}
