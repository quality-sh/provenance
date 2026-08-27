import assert from "node:assert/strict";
import { execFileSync, spawn, spawnSync } from "node:child_process";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { delimiter, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const sleep = new Int32Array(new SharedArrayBuffer(4));
const fixture = fileURLToPath(
  new URL("../test/fixtures/synthetic-ste-dictionary.pdf", import.meta.url),
);
const fixtureServer = fileURLToPath(new URL("./packed-ste-fixture-server.js", import.meta.url));
const rejectedStatement = "The bunapprovedaaa item stops.";

export function verifyPackedSteOnboarding({
  archiveDirectory,
  binaryName,
  engineArchive,
  engineManifest,
  initializerArchive,
  initializerManifest,
  isolatedCache,
  mainArchive,
  npmCli,
  temporary,
  typescriptArchive,
  typescriptManifest,
  version,
}) {
  const project = join(temporary, "ste-onboarding-application");
  const packedMainSpec = `file:../archives/${mainArchive}`;
  mkdirSync(project);
  writeFileSync(join(project, "package.json"), JSON.stringify({
    name: "provenance-ste-onboarding-release-gate",
    private: true,
    version: "1.0.0",
    type: "module",
    packageManager: "npm@11.6.2",
    overrides: {
      [engineManifest.name]: `file:../archives/${engineArchive}`,
      "@quality-sh/provenance": packedMainSpec,
      [typescriptManifest.name]: `file:../archives/${typescriptArchive}`,
    },
  }));
  npm(npmCli, [
    "install",
    "--offline",
    "--cache", isolatedCache,
    "--no-audit",
    "--no-fund",
    "--no-save",
    join(archiveDirectory, initializerArchive),
  ], project);

  const server = startFixtureServer(temporary);
  const sentinel = createStaleGlobal(temporary);
  const environment = isolatedEnvironment({
    assetUrl: server.assetUrl,
    isolatedCache,
    packedMainSpec,
    sentinelDirectory: sentinel.directory,
    temporary,
  });
  const initializer = join(
    project,
    "node_modules",
    ...initializerManifest.name.split("/"),
    initializerManifest.bin["create-provenance"],
  );

  try {
    execFileSync(process.execPath, [initializer, "--ste-onboarding", "agent"], {
      cwd: project,
      env: environment,
      stdio: "pipe",
    });
    assertSingleDictionaryRequest(server.requests());

    const packageLocalEntry = join(
      project, "node_modules", "@quality-sh", "provenance", "bin", "provenance.mjs",
    );
    execFileSync(process.execPath, [
      packageLocalEntry, "init", "--path", ".", "--ste-onboarding", "agent",
    ], { cwd: project, env: environment, stdio: "pipe" });
    assertSingleDictionaryRequest(server.requests());

    assertInitializedPackage(project, packedMainSpec, initializerManifest);
    assertAgentInstructions(project);
    assertLocalEngines(project, environment, engineManifest, binaryName, version, npmCli);
    assertPreflightAndWriteGate(project, environment, npmCli);
    assertStrictCommittedEditGate(project, environment, npmCli, version);
    assertSingleDictionaryRequest(server.requests());
    assert.equal(existsSync(sentinel.selected), false, "the stale global CLI must stay unused");
  } finally {
    server.stop();
  }
}

function assertInitializedPackage(project, packedMainSpec, initializerManifest) {
  const manifest = JSON.parse(readFileSync(join(project, "package.json"), "utf8"));
  assert.equal(manifest.devDependencies["@quality-sh/provenance"], packedMainSpec);
  assert.equal(manifest.dependencies?.[initializerManifest.name], undefined);
  assert.equal(readFileSync(join(project, ".gitignore"), "utf8"), ".provenance/cache/\n");
  assert.ok(existsSync(join(project, ".provenance", "state", "dictionary.json")));
}

function assertAgentInstructions(project) {
  const agents = readFileSync(join(project, "AGENTS.md"), "utf8");
  for (const expected of [
    "provenance-grounded-writing",
    "npx --no provenance sdk check-statement --format json",
    "npx --no provenance prime --quiet",
    "Write graph state only through the Provenance CLI or SDK",
    "Do not edit\n  `.provenance/state` directly",
    "ASD owns ASD-STE100. STEMG maintains it",
    "covers only the\n  ASD-STE100 Issue 9 checks that Provenance implements",
    "It does not prove full\n  conformance",
  ]) {
    assert.ok(agents.includes(expected), `AGENTS.md must contain ${JSON.stringify(expected)}`);
  }

  const installed = readFileSync(
    join(project, ".agents", "skills", "provenance-grounded-writing", "SKILL.md"),
    "utf8",
  );
  const compatible = readFileSync(
    join(project, ".claude", "skills", "provenance-grounded-writing", "SKILL.md"),
    "utf8",
  );
  assert.match(installed, /name: provenance-grounded-writing/);
  assert.match(installed, /Write specific, evidence-grounded statements/);
  assert.equal(compatible, installed, "both agent skill roots must expose grounded writing");
}

function assertLocalEngines(project, environment, engineManifest, binaryName, version, npmCli) {
  const platformBinary = join(
    project,
    "node_modules",
    ...engineManifest.name.split("/"),
    "bin",
    binaryName,
  );
  const directVersion = execFileSync(platformBinary, ["--version"], {
    cwd: project,
    env: environment,
    encoding: "utf8",
  });
  assert.equal(directVersion.trim(), `provenance ${version}`);

  const check = runProvenance(npmCli, project, environment, [
    "check", "--repo", ".", "--format", "json",
  ]);
  assert.equal(check.status, 0, check.stderr);
  assert.equal(JSON.parse(check.stdout).status, "ok");
}

function assertPreflightAndWriteGate(project, environment, npmCli) {
  const preflight = runProvenance(
    npmCli,
    project,
    environment,
    ["sdk", "check-statement", "--format", "json"],
    JSON.stringify({ statement: "Stop; wait." }),
  );
  assert.equal(preflight.status, 0, preflight.stderr);
  const report = JSON.parse(preflight.stdout);
  assert.ok(report.findings.some(({ rule, kind }) => rule === "8.1" && kind === "violation"));

  const shard = join(
    project,
    ".provenance", "state", "scopes", "default", "requirements", "req.jsonl",
  );
  const beforeWrite = existsSync(shard) ? readFileSync(shard, "utf8") : undefined;
  const rejected = runProvenance(npmCli, project, environment, [
    "requirements", "create",
    "--repo", ".",
    "--scope", "default",
    "--id", "req_rejected_by_dictionary",
    "--statement", rejectedStatement,
    "--format", "json",
  ]);
  assert.notEqual(rejected.status, 0, "the real CLI must reject the unapproved word");
  const error = JSON.parse(rejected.stderr.trim().replace(/^Error: /, ""));
  assert.equal(error.field, "statement");
  assert.ok(error.findings.some(({ rule }) => rule === "1.1"));
  const afterWrite = existsSync(shard) ? readFileSync(shard, "utf8") : undefined;
  assert.equal(afterWrite, beforeWrite, "a rejected write must not change requirement state");
}

function assertStrictCommittedEditGate(project, environment, npmCli, version) {
  git(project, ["init", "--initial-branch", "main"]);
  git(project, ["config", "user.email", "packed@example.test"]);
  git(project, ["config", "user.name", "Packed release gate"]);
  const created = runProvenance(npmCli, project, environment, [
    "requirements", "create",
    "--repo", ".",
    "--scope", "default",
    "--id", "req_packed_manual_edit",
    "--statement", "Install the cover.",
    "--format", "json",
  ]);
  assert.equal(created.status, 0, created.stderr);
  const base = commit(project, "Create a clean Provenance project");

  const shard = join(
    project,
    ".provenance", "state", "scopes", "default", "requirements", "req.jsonl",
  );
  const rewritten = readFileSync(shard, "utf8")
    .trimEnd()
    .split("\n")
    .map((line) => {
      const record = JSON.parse(line);
      if (record.id === "req_packed_manual_edit") record.statement = rejectedStatement;
      return JSON.stringify(record);
    })
    .join("\n") + "\n";
  writeFileSync(shard, rewritten);
  const candidate = commit(project, "Commit a bad manual statement edit");

  const strict = runProvenance(npmCli, project, environment, [
    "check", "--repo", ".", "--strict", "--base", base, "--format", "json",
  ]);
  assert.notEqual(strict.status, 0, "the project-local strict CI command must block findings");
  const report = JSON.parse(strict.stdout);
  assert.equal(report.status, "findings");
  assert.equal(report.base_commit, base);
  assert.equal(report.candidate_commit, candidate);
  assert.deepEqual(report.diagnostics, [{
    resource_kind: "requirement",
    scope_id: "default",
    id: "req_packed_manual_edit",
    field: "statement",
    standard: "ASD-STE100",
    issue: 9,
    analyzer_version: version,
    rule: "1.1",
    disposition: "violation",
    span: { start: 4, end: 18 },
    message: "Do not use unapproved dictionary words in descriptive text.",
  }]);
}

function runProvenance(npmCli, cwd, env, args, input) {
  const result = spawnSync(process.execPath, [
    join(dirname(npmCli), "npx-cli.js"), "--no", "provenance", ...args,
  ], { cwd, env, input, encoding: "utf8" });
  if (result.error) throw result.error;
  return result;
}

function npm(npmCli, args, cwd) {
  execFileSync(process.execPath, [npmCli, ...args], { cwd, stdio: "pipe" });
}

function git(cwd, args) {
  const result = spawnSync("git", args, { cwd, encoding: "utf8" });
  assert.equal(result.status, 0, `git ${args.join(" ")} failed: ${result.stderr}`);
  return result.stdout.trim();
}

function commit(project, message) {
  git(project, ["add", "."]);
  git(project, ["commit", "-m", message]);
  return git(project, ["rev-parse", "HEAD"]);
}

function createStaleGlobal(temporary) {
  const directory = join(temporary, "stale-global-ste-gate");
  const selected = join(directory, "selected");
  const command = join(directory, process.platform === "win32" ? "provenance.cmd" : "provenance");
  mkdirSync(directory);
  writeFileSync(
    command,
    process.platform === "win32"
      ? `@echo stale>"${selected}"\r\n@exit /b 42\r\n`
      : `#!/bin/sh\nprintf stale > '${selected}'\nexit 42\n`,
  );
  if (process.platform !== "win32") chmodSync(command, 0o755);
  return { directory, selected };
}

function isolatedEnvironment({ assetUrl, isolatedCache, packedMainSpec, sentinelDirectory, temporary }) {
  const {
    ALL_PROXY: _allProxy,
    HTTPS_PROXY: _httpsProxy,
    HTTP_PROXY: _httpProxy,
    PROVENANCE_BIN: _provenanceBin,
    all_proxy: _allProxyLower,
    https_proxy: _httpsProxyLower,
    http_proxy: _httpProxyLower,
    ...inherited
  } = process.env;
  return {
    ...inherited,
    ALL_PROXY: "http://127.0.0.1:9",
    HTTPS_PROXY: "http://127.0.0.1:9",
    HTTP_PROXY: "http://127.0.0.1:9",
    all_proxy: "http://127.0.0.1:9",
    https_proxy: "http://127.0.0.1:9",
    http_proxy: "http://127.0.0.1:9",
    PATH: `${sentinelDirectory}${delimiter}${process.env.PATH ?? ""}`,
    NO_PROXY: "127.0.0.1,localhost",
    no_proxy: "127.0.0.1,localhost",
    npm_config_cache: isolatedCache,
    npm_config_offline: "true",
    npm_config_registry: "http://127.0.0.1:9",
    npm_config_update_notifier: "false",
    PROVENANCE_PACKAGE_SPEC: packedMainSpec,
    PROVENANCE_STE100_ASSET_DIR: join(temporary, "ste-assets"),
    PROVENANCE_STE100_INDEX_DIR: join(temporary, "ste-indexes"),
    PROVENANCE_TEST_STE100_ASSET_URL: assetUrl,
  };
}

function startFixtureServer(temporary) {
  const ready = join(temporary, "ste-server-ready.json");
  const count = join(temporary, "ste-server-count");
  const child = spawn(process.execPath, [fixtureServer, fixture, ready, count], {
    stdio: ["ignore", "pipe", "pipe"],
  });
  const deadline = Date.now() + 10_000;
  while (!existsSync(ready) && Date.now() < deadline) Atomics.wait(sleep, 0, 0, 25);
  assert.ok(existsSync(ready), "the loopback dictionary fixture must start");
  const { port } = JSON.parse(readFileSync(ready, "utf8"));
  return {
    assetUrl: `http://127.0.0.1:${port}/ASD-STE100_ISSUE9.pdf`,
    requests: () => JSON.parse(readFileSync(count, "utf8")),
    stop: () => child.kill(),
  };
}

function assertSingleDictionaryRequest(requests) {
  assert.deepEqual(requests, [{ method: "GET", url: "/ASD-STE100_ISSUE9.pdf" }]);
}
