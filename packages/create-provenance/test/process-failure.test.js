import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import {
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

// @provenance verification: examples
// @provenance rule: rule_typescript_initializer_validates_project
test("the command reports a real installed-engine init failure", () => {
  const temporary = mkdtempSync(join(tmpdir(), "create-provenance-failure-"));
  try {
    const sdk = join(temporary, "sdk");
    const project = join(temporary, "project");
    mkdirSync(join(sdk, "bin"), { recursive: true });
    mkdirSync(join(sdk, "dist"));
    mkdirSync(project);
    writeFileSync(join(sdk, "package.json"), JSON.stringify({
      name: "@quality-sh/provenance",
      version: "0.2.2",
      type: "module",
      main: "dist/index.js",
      bin: { provenance: "bin/provenance.mjs" },
    }));
    writeFileSync(join(sdk, "dist/index.js"), "export {};\n");
    writeFileSync(
      join(sdk, "bin/provenance.mjs"),
      "#!/usr/bin/env node\nif (process.argv[2] === 'init') process.exitCode = 23;\n",
    );
    const npmCli = process.env.npm_execpath;
    assert.ok(npmCli, "run this test through npm");
    const packed = JSON.parse(execFileSync(
      process.execPath,
      [npmCli, "pack", sdk, "--json"],
      { cwd: temporary, encoding: "utf8" },
    ))[0].filename;
    writeFileSync(join(project, "package.json"), JSON.stringify({
      name: "failure-fixture",
      private: true,
      packageManager: `npm@${execFileSync("npm", ["--version"], { encoding: "utf8" }).trim()}`,
    }));

    const result = spawnSync(
      process.execPath,
      [fileURLToPath(new URL("../bin/create-provenance.mjs", import.meta.url))],
      {
        cwd: project,
        encoding: "utf8",
        env: {
          ...process.env,
          PROVENANCE_PACKAGE_SPEC: join(temporary, packed),
        },
      },
    );

    assert.equal(result.status, 1);
    assert.match(result.stderr, /Provenance initialization failed with exit code 23/);
    const manifest = JSON.parse(readFileSync(join(project, "package.json"), "utf8"));
    assert.ok(manifest.devDependencies["@quality-sh/provenance"]);
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});
