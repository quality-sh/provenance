// Bun regression coverage for the calling frame Bun eliminates. Node keeps that
// frame, so this shape only reproduces under Bun: `npm run test:bun`.
import { chmodSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { expect, test } from "bun:test";

// Answers the protocol handshake and reports every other request on stderr.
function reportingEngine(): string {
  const executable = join(mkdtempSync(join(tmpdir(), "provenance-bun-engine-")), "engine.mjs");
  writeFileSync(
    executable,
    `#!/usr/bin/env node
import { readFileSync } from "node:fs";
const command = process.argv[3];
if (command === "info") {
  process.stdout.write(JSON.stringify({
    engine_version: "0.1.0",
    protocol_version: 5,
    state_schema_version: 1,
    repository: "/project",
  }));
} else {
  process.stderr.write("ENGINE " + command + " " + readFileSync(0, "utf8"));
  process.exit(3);
}
`,
  );
  chmodSync(executable, 0o755);
  return executable;
}

function runCase(statedFile: boolean): string {
  const result = Bun.spawnSync({
    cmd: [process.execPath, "test", "./tail-call-case.ts"],
    cwd: import.meta.dir,
    env: {
      ...process.env,
      PROVENANCE_TEST_ENGINE: reportingEngine(),
      PROVENANCE_STATED_FILE: statedFile ? "1" : "0",
    },
  });
  return result.stdout.toString() + result.stderr.toString();
}

test("a verify call Bun cannot place says which file to state", () => {
  const output = runCase(false);

  expect(output).toContain("import.meta.path");
  expect(output).toContain("share-link-expiry");
  expect(output).not.toContain("ENGINE begin-verification");
});

test("import.meta states the file Bun cannot report", () => {
  const output = runCase(true);

  expect(output).not.toContain("import.meta.path");
  expect(output).toContain("ENGINE begin-verification");
  expect(output).toContain(`"file":"${join(import.meta.dir, "tail-call-case.ts")}"`);
});
