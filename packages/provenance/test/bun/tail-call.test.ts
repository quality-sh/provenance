// Bun removes the caller frame for the tail position below. Node does not.
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "bun:test";
import { startFixtureHost } from "../../scripts/fixture-host.js";

const cli = fileURLToPath(new URL(`../../../../target/debug/provenance${process.platform === "win32" ? ".exe" : ""}`, import.meta.url));
const sdk = new URL("../../dist/index.js", import.meta.url).href;

async function runCase(statedFile: boolean) {
  const root = mkdtempSync(join(tmpdir(), "provenance-bun-tail-call-"));
  try {
    execFileSync(cli, ["--quiet", "init", "--path", root, "--scope", "default", "--path-prefix", "."], { stdio: "pipe" });
    const source = readFileSync(new URL("./tail-call-case.ts", import.meta.url), "utf8");
    writeFileSync(join(root, "tail-call-case.ts"), source.replace("../../dist/index.js", sdk));
    const fixture = await startFixtureHost({ root, repositoryId: "bun-tail-call" });
    let output;
    try {
      const result = Bun.spawnSync({
        cmd: [process.execPath, "test", "./tail-call-case.ts"],
        cwd: root,
        env: { ...process.env, ...fixture.environment, PROVENANCE_STATED_FILE: statedFile ? "1" : "0" },
      });
      output = result.stdout.toString() + result.stderr.toString();
      if (statedFile && result.exitCode !== 0) throw new Error(output);
    } finally { await fixture.close(); }
    const runs = JSON.parse(execFileSync(cli, ["sdk", "verification-runs", "--repo", root, "--scope", "default", "--format", "json"], { encoding: "utf8" }));
    return { output, runs };
  } finally { rmSync(root, { recursive: true, force: true }); }
}

test("a verify call Bun cannot place says which file to state", async () => {
  const { output, runs } = await runCase(false);
  expect(output).toContain("import.meta.path");
  expect(output).toContain("share-link-expiry");
  expect(runs).toHaveLength(0);
});

test("import.meta states the real file and completes verification through HTTP", async () => {
  const { output, runs } = await runCase(true);
  expect(output).not.toContain("import.meta.path");
  expect(runs).toHaveLength(1);
  expect(runs[0].status).toBe("passed");
});
