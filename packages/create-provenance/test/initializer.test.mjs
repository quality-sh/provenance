import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test, { after } from "node:test";

import * as initializer from "../src/initializer.mjs";

const { initializeProject, parseArguments } = initializer;

const packageVersion = "0.1.0";
const packageSpec = `@quality-sh/provenance@${packageVersion}`;
const temporaryDirectories = [];

after(() => {
  for (const directory of temporaryDirectories) {
    rmSync(directory, { recursive: true, force: true });
  }
});

test("the initializer has no runtime dependency on the SDK it installs", () => {
  const manifest = JSON.parse(
    readFileSync(new URL("../package.json", import.meta.url), "utf8"),
  );

  assert.equal(manifest.dependencies, undefined);
});

const managers = [
  ["npm", "npm", ["install", "--save-dev", "--save-exact", packageSpec]],
  ["pnpm", "pnpm", ["add", "--save-dev", "--save-exact", packageSpec]],
  ["yarn", "yarn", ["add", "--dev", "--exact", packageSpec]],
  ["bun", "bun", ["add", "--dev", "--exact", packageSpec]],
  [
    "deno",
    "deno",
    [
      "add",
      "--dev",
      "--save-exact",
      "--package-json",
      "--minimum-dependency-age=0",
      `npm:${packageSpec}`,
    ],
  ],
  [
    "nub",
    "nub",
    [
      "add",
      "-D",
      "-E",
      "--allow-low-downloads",
      "--minimum-release-age-exclude",
      "@quality-sh/provenance*",
      packageSpec,
    ],
  ],
];

for (const [manager, command, args] of managers) {
  test(`${manager} installs Provenance as an exact development dependency`, () => {
    const project = projectDirectory({ packageManager: `${manager}@1.0.0` });
    const invocations = [];

    const result = initializeProject({
      projectDirectory: project,
      packageVersion,
      enginePath: "/provenance-engine",
      execute: recordingExecutor(invocations),
    });

    assert.equal(result.packageManager, manager);
    assert.deepEqual(invocations[0], { command, args, capture: false });
    assert.deepEqual(invocations.slice(1), [
      {
        command: "/provenance-engine",
        args: ["init", "--path", project, "--scope", "default", "--path-prefix", "."],
        capture: false,
      },
      {
        command: "/provenance-engine",
        args: ["check", "--repo", project, "--format", "json"],
        capture: true,
      },
    ]);
    assert.equal(
      readFileSync(join(project, ".gitignore"), "utf8"),
      ".provenance/cache/\n",
    );
  });
}

const lockfiles = [
  ["package-lock.json", "npm"],
  ["pnpm-lock.yaml", "pnpm"],
  ["yarn.lock", "yarn"],
  ["bun.lock", "bun"],
  ["deno.lock", "deno"],
  ["nub.lock", "nub"],
];

for (const [lockfile, manager] of lockfiles) {
  test(`${lockfile} selects ${manager}`, () => {
    const project = projectDirectory();
    writeFileSync(join(project, lockfile), "");

    const result = initializeProject({
      projectDirectory: project,
      packageVersion,
      enginePath: "/provenance-engine",
      execute: recordingExecutor([]),
    });

    assert.equal(result.packageManager, manager);
  });
}

test("conflicting lockfiles require an explicit package manager", () => {
  const project = projectDirectory();
  writeFileSync(join(project, "package-lock.json"), "");
  writeFileSync(join(project, "bun.lock"), "");

  assert.throws(
    () => initializeProject({
      projectDirectory: project,
      packageVersion,
      enginePath: "/provenance-engine",
      execute: recordingExecutor([]),
    }),
    /More than one package manager is present.*--package-manager/s,
  );
});

test("an existing cache ignore entry is not duplicated", () => {
  const project = projectDirectory({ packageManager: "npm@11.0.0" });
  writeFileSync(join(project, ".gitignore"), "dist/\n.provenance/cache/\n");

  initializeProject({
    projectDirectory: project,
    packageVersion,
    enginePath: "/provenance-engine",
    execute: recordingExecutor([]),
  });

  assert.equal(
    readFileSync(join(project, ".gitignore"), "utf8"),
    "dist/\n.provenance/cache/\n",
  );
});

test("a failed validation does not claim success or edit gitignore", () => {
  const project = projectDirectory({ packageManager: "npm@11.0.0" });

  assert.throws(
    () => initializeProject({
      projectDirectory: project,
      packageVersion,
      enginePath: "/provenance-engine",
      execute({ capture }) {
        return capture
          ? { status: 0, stdout: JSON.stringify({ status: "error" }) }
          : { status: 0, stdout: "" };
      },
    }),
    /freshly initialized project did not validate/,
  );
  assert.throws(() => readFileSync(join(project, ".gitignore")), /ENOENT/);
});

test("the command defaults to the current project", () => {
  assert.deepEqual(parseArguments([], "/workspace/application"), {
    projectDirectory: "/workspace/application",
    packageManager: undefined,
  });
});

test("the command accepts a target path and package-manager override", () => {
  assert.deepEqual(
    parseArguments(["--path", "packages/web", "--package-manager", "bun"], "/workspace"),
    {
      projectDirectory: "/workspace/packages/web",
      packageManager: "bun",
    },
  );
});

test("the command rejects unknown arguments", () => {
  assert.throws(
    () => parseArguments(["--install"], "/workspace"),
    /Unknown argument '--install'/,
  );
});

test("a development package override replaces the registry package spec", () => {
  const project = projectDirectory({ packageManager: "npm@11.0.0" });
  const invocations = [];

  initializeProject({
    projectDirectory: project,
    packageVersion,
    packageSpec: "file:../archives/quality-sh-provenance.tgz",
    enginePath: "/provenance-engine",
    execute: recordingExecutor(invocations),
  });

  assert.deepEqual(invocations[0].args, [
    "install",
    "--save-dev",
    "--save-exact",
    "file:../archives/quality-sh-provenance.tgz",
  ]);
});

test("the installed SDK engine is resolved after dependency installation", () => {
  const project = projectDirectory({ packageManager: "npm@11.0.0" });
  const invocations = [];
  let resolvedAfterInstallation = false;

  initializeProject({
    projectDirectory: project,
    packageVersion,
    resolveEngine(directory) {
      assert.equal(directory, project);
      resolvedAfterInstallation = invocations.length === 1;
      return {
        command: "/usr/bin/node",
        args: ["/project/node_modules/@quality-sh/provenance/bin/provenance.mjs"],
      };
    },
    execute: recordingExecutor(invocations),
  });

  assert.equal(resolvedAfterInstallation, true);
  assert.deepEqual(invocations[1].args.slice(0, 2), [
    "/project/node_modules/@quality-sh/provenance/bin/provenance.mjs",
    "init",
  ]);
  assert.deepEqual(invocations[2].args.slice(0, 2), [
    "/project/node_modules/@quality-sh/provenance/bin/provenance.mjs",
    "check",
  ]);
});

test("Yarn Plug'n'Play resolves the installed SDK through its project loader", () => {
  const project = projectDirectory({ packageManager: "yarn@4.9.0" });
  const sdkEntry = join(project, ".yarn", "cache", "provenance", "dist", "index.js");
  writeFileSync(
    join(project, ".pnp.cjs"),
    `module.exports.resolveRequest = () => ${JSON.stringify(sdkEntry)};\n`,
  );
  const invocations = [];

  initializeProject({
    projectDirectory: project,
    packageVersion,
    execute: recordingExecutor(invocations),
  });

  assert.equal(invocations[1].command, "yarn");
  assert.deepEqual(invocations[1].args.slice(0, 2), [
    "node",
    join(project, ".yarn", "cache", "provenance", "bin", "provenance.mjs"),
  ]);
});

test("Windows package-manager shims run through the command interpreter", () => {
  assert.deepEqual(
    initializer.hostInvocation(
      {
        command: "npm",
        args: ["install", "--save-dev", packageSpec],
        capture: false,
      },
      "win32",
      "C:\\Windows\\System32\\cmd.exe",
    ),
    {
      command: "C:\\Windows\\System32\\cmd.exe",
      args: ["/d", "/s", "/c", "npm", "install", "--save-dev", packageSpec],
      capture: false,
    },
  );
});

test("Windows runs native engine executables directly", () => {
  const invocation = {
    command: "C:\\Program Files\\nodejs\\node.exe",
    args: ["C:\\project\\provenance.mjs", "init"],
    capture: false,
  };

  assert.deepEqual(
    initializer.hostInvocation(
      invocation,
      "win32",
      "C:\\Windows\\System32\\cmd.exe",
    ),
    invocation,
  );
});

function projectDirectory(manifest = {}) {
  const directory = mkdtempSync(join(tmpdir(), "create-provenance-test-"));
  temporaryDirectories.push(directory);
  writeFileSync(join(directory, "package.json"), JSON.stringify({
    name: "initializer-fixture",
    private: true,
    ...manifest,
  }));
  return directory;
}

function recordingExecutor(invocations) {
  return (invocation) => {
    invocations.push(invocation);
    return invocation.capture
      ? { status: 0, stdout: JSON.stringify({ status: "ok" }) }
      : { status: 0, stdout: "" };
  };
}
