// Implementation aid: no Rule states where the example lists its Provenance
// package. The script shows that the built application loads in an install that
// omits development dependencies: it installs a copy of the package files and
// built output with `npm ci --omit=dev`, checks that `@quality-sh/provenance`
// does not resolve there, and loads the production module from that copy.
//
// It loads only `dist/share-links.js`, the one production module. No file-name
// rule tells the production modules from `dist/apply.js`, a development script
// that imports the package, so a second production module that imports the
// package is not found here.
//
// `npm run test:production` starts this script. npm gives the path of its own
// command-line script in `npm_execpath`, and Node runs that script on each
// platform with no shell.
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
  const npm = process.env.npm_execpath;
  assert.ok(npm, "start this script with `npm run test:production`");
  const ci = spawnSync(
    process.execPath,
    [npm, "ci", "--omit=dev", "--ignore-scripts", "--no-audit", "--no-fund"],
    { cwd: install, encoding: "utf8" },
  );
  assert.equal(ci.status, 0, ci.error?.message ?? ci.stderr);

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
