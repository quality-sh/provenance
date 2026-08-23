import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test, { after } from "node:test";

import { initializeProject, parseArguments } from "../src/initializer.mjs";

const packageVersion = "0.1.0";
const packageSpec = `@quality-sh/provenance@${packageVersion}`;
const temporaryDirectories = [];

after(() => {
  for (const directory of temporaryDirectories) {
    rmSync(directory, { recursive: true, force: true });
  }
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
      "--minimum-release-age=0",
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

test("the engine command can run through the packaged JavaScript entry point", () => {
  const project = projectDirectory({ packageManager: "npm@11.0.0" });
  const invocations = [];

  initializeProject({
    projectDirectory: project,
    packageVersion,
    enginePath: "/usr/bin/node",
    engineArguments: ["/sdk/bin/provenance.mjs"],
    execute: recordingExecutor(invocations),
  });

  assert.deepEqual(invocations[1].args.slice(0, 2), [
    "/sdk/bin/provenance.mjs",
    "init",
  ]);
  assert.deepEqual(invocations[2].args.slice(0, 2), [
    "/sdk/bin/provenance.mjs",
    "check",
  ]);
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
