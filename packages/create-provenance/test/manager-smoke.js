import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const manager = process.env.PROVENANCE_TEST_PACKAGE_MANAGER;
assert.ok(manager, "set PROVENANCE_TEST_PACKAGE_MANAGER");

const packageRoot = fileURLToPath(new URL("..", import.meta.url));
const npmCli = process.env.npm_execpath;
assert.ok(npmCli, "run this test through npm so its CLI path is known");

const temporary = mkdtempSync(join(tmpdir(), `create-provenance-${manager}-`));
process.once("exit", () => rmSync(temporary, { recursive: true, force: true }));

const sdkRoot = join(temporary, "sdk");
const project = join(temporary, "project");
mkdirSync(join(sdkRoot, "bin"), { recursive: true });
mkdirSync(join(sdkRoot, "dist"), { recursive: true });
mkdirSync(project);

writeFileSync(join(sdkRoot, "package.json"), JSON.stringify({
  name: "@quality-sh/provenance",
  version: "0.1.0",
  type: "module",
  main: "dist/index.js",
  bin: { provenance: "bin/provenance.mjs" },
  files: ["bin", "dist"],
}));
writeFileSync(join(sdkRoot, "dist", "index.js"), "export {};\n");
writeFileSync(join(sdkRoot, "bin", "provenance.mjs"), `#!/usr/bin/env node
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const [operation, ...args] = process.argv.slice(2);
const value = (name) => args[args.indexOf(name) + 1];
if (operation === "init") {
  const root = value("--path");
  mkdirSync(join(root, ".provenance", "state"), { recursive: true });
  writeFileSync(join(root, ".provenance", "state", "manifest.json"), "{}\\n");
} else if (operation === "check") {
  process.stdout.write('{"status":"ok"}');
} else {
  process.exitCode = 2;
}
`);

const packOutput = execFileSync(process.execPath, [npmCli, "pack", sdkRoot, "--json"], {
  cwd: temporary,
  encoding: "utf8",
  stdio: ["ignore", "pipe", "pipe"],
});
const archive = join(temporary, JSON.parse(packOutput)[0].filename);
const sdkSpec = manager === "deno"
  ? "@quality-sh/provenance@0.1.0"
  : archive;
const versionOutput = execFileSync(manager, ["--version"], { encoding: "utf8" });
const managerVersion = manager === "deno"
  ? versionOutput.match(/^deno\s+(\S+)/)?.[1]
  : versionOutput.trim();
assert.ok(managerVersion, `could not read the ${manager} version`);
writeFileSync(join(project, "package.json"), JSON.stringify({
  name: `${manager}-initializer-smoke`,
  private: true,
  packageManager: `${manager}@${managerVersion}`,
}));

execFileSync(process.execPath, [join(packageRoot, "bin", "create-provenance.mjs")], {
  cwd: project,
  env: {
    ...process.env,
    PROVENANCE_PACKAGE_SPEC: sdkSpec,
  },
  stdio: "pipe",
});

verifyManagerInstall(manager, project);

// @provenance rule: rule_typescript_initializer_installs_dev_dependency
// @provenance verification: examples
function verifyManagerInstall(packageManager, directory) {
  const manifest = JSON.parse(readFileSync(join(directory, "package.json"), "utf8"));
  assert.ok(
    manifest.devDependencies?.["@quality-sh/provenance"],
    `${packageManager} must save Provenance as a development dependency`,
  );
  assert.equal(
    manifest.dependencies?.["@quality-sh/create-provenance"],
    undefined,
    "the temporary initializer must not become a project dependency",
  );
  assert.equal(
    readFileSync(join(directory, ".gitignore"), "utf8"),
    ".provenance/cache/\n",
  );
  assert.equal(installedSdkVersion(directory), "0.1.0");
}

function installedSdkVersion(directory) {
  const pnpPath = join(directory, ".pnp.cjs");
  if (existsSync(pnpPath)) {
    return execFileSync("yarn", [
      "node",
      "-p",
      "require('@quality-sh/provenance/package.json').version",
    ], { cwd: directory, encoding: "utf8" }).trim();
  }
  const entry = createRequire(join(directory, "package.json"))
    .resolve("@quality-sh/provenance");
  return JSON.parse(readFileSync(join(dirname(dirname(entry)), "package.json"), "utf8"))
    .version;
}
