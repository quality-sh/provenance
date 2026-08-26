import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { delimiter, join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const publishScript = fileURLToPath(
  new URL("../../../.github/scripts/publish-npm-if-missing.sh", import.meta.url),
);

function invokeWithFakeNpm(context, behavior) {
  const temporary = mkdtempSync(join(tmpdir(), "provenance-publish-npm-"));
  context.after(() => rmSync(temporary, { recursive: true, force: true }));
  const callLog = join(temporary, "calls.log");
  const npm = join(temporary, "npm");
  writeFileSync(npm, `#!/bin/sh
printf '%s\\n' "$*" >> "$NPM_CALL_LOG"
${behavior}
`, { mode: 0o755 });

  const output = execFileSync("bash", [
    publishScript,
    "@quality-sh/provenance",
    "0.2.0",
    "./quality-sh-provenance-0.2.0.tgz",
    "latest",
  ], {
    env: {
      ...process.env,
      NPM_CALL_LOG: callLog,
      PATH: `${temporary}${delimiter}${process.env.PATH}`,
    },
    encoding: "utf8",
  });

  return { callLog: readFileSync(callLog, "utf8"), output };
}

test("an existing npm version is not published again", (context) => {
  const result = invokeWithFakeNpm(context, `
if [ "$1" = "view" ]; then
  printf '"0.2.0"\\n'
  exit 0
fi
exit 97`);

  assert.equal(
    result.callLog,
    "view @quality-sh/provenance@0.2.0 version --json\n",
  );
});

test("a missing npm version is published", (context) => {
  const result = invokeWithFakeNpm(context, `
if [ "$1" = "view" ]; then
  echo "npm error code E404" >&2
  exit 1
fi
if [ "$1" = "publish" ]; then
  exit 0
fi
exit 97`);

  assert.equal(
    result.callLog,
    "view @quality-sh/provenance@0.2.0 version --json\n" +
      "publish ./quality-sh-provenance-0.2.0.tgz --access public --provenance --tag latest\n",
  );
});

test("every release package uses recovery-safe npm publication", () => {
  const workflow = readFileSync(
    fileURLToPath(new URL("../../../.github/workflows/release.yml", import.meta.url)),
    "utf8",
  );
  assert.equal(
    workflow.match(/publish-npm-if-missing\.sh/g)?.length,
    3,
    "platform engines, the SDK, and the initializer must all use the helper",
  );
  assert.doesNotMatch(workflow, /\bnpm publish\b/);
});
