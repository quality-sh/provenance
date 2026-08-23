import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const packageRoot = fileURLToPath(new URL("..", import.meta.url));
const repositoryRoot = fileURLToPath(new URL("../../..", import.meta.url));
const npmCli = process.env.npm_execpath;
assert.ok(npmCli, "run this test through npm so its CLI path is known");
const npxCli = join(dirname(npmCli), "npx-cli.js");
const version = JSON.parse(readFileSync(join(packageRoot, "package.json"), "utf8")).version;
const temporary = mkdtempSync(join(tmpdir(), "provenance-packed-install-"));
process.once("exit", () => rmSync(temporary, { recursive: true, force: true }));
const stagedEngine = join(temporary, "engine-package");
const archiveDirectory = join(temporary, "archives");
const isolatedCache = join(temporary, "npm-cache");
const rustTarget = targetFor(process.platform, process.arch);
const binaryName = process.platform === "win32" ? "provenance.exe" : "provenance";
const builtBinary = join(repositoryRoot, "target", "debug", binaryName);
const typescriptRoot = join(packageRoot, "node_modules", "typescript");
const typescriptManifest = JSON.parse(
  readFileSync(join(typescriptRoot, "package.json"), "utf8"),
);

mkdirSync(archiveDirectory);
execFileSync(process.execPath, [
  join(packageRoot, "scripts", "package-engine.js"),
  "--target", rustTarget,
  "--binary", builtBinary,
  "--out", stagedEngine,
  "--version", version,
]);
npm(["pack", stagedEngine, "--pack-destination", archiveDirectory]);
npm(["pack", packageRoot, "--pack-destination", archiveDirectory]);
npm(["pack", typescriptRoot, "--pack-destination", archiveDirectory]);

const archives = readdirSync(archiveDirectory).filter((name) => name.endsWith(".tgz"));
const mainArchive = archiveNamed(archives, `quality-sh-provenance-${version}.tgz`);
const engineManifest = JSON.parse(readFileSync(join(stagedEngine, "package.json"), "utf8"));
const engineArchive = archiveNamed(archives, archiveName(engineManifest));
const typescriptArchive = archiveNamed(archives, archiveName(typescriptManifest));

// The quick start in README.md installs one package and lets its optional
// platform dependency bring the engine. There is no registry here, so
// `overrides` points every download at a local archive and nothing else
// changes: the install still asks for @quality-sh/provenance alone.
const application = join(temporary, "application");
mkdirSync(application);
writeFileSync(join(application, "package.json"), JSON.stringify({
  name: "provenance-clean-install",
  private: true,
  version: "1.0.0",
  type: "module",
  overrides: {
    [engineManifest.name]: `file:../archives/${engineArchive}`,
    [typescriptManifest.name]: `file:../archives/${typescriptArchive}`,
  },
}));
npm(
  [
    "install",
    "--offline",
    "--cache",
    isolatedCache,
    "--no-audit",
    "--no-fund",
    join(archiveDirectory, mainArchive),
  ],
  { cwd: application },
);

// The second line of the quick start, run as written through the installed bin.
provenance(["init", "--path", ".", "--scope", "default", "--path-prefix", "."], application);
const projectManifest = JSON.parse(
  readFileSync(join(application, ".provenance", "state", "manifest.json"), "utf8"),
);
assert.deepEqual(
  projectManifest.scopes,
  [{ id: "default", path_prefix: "." }],
  "init through the installed bin must record the scope the quick start asks for",
);
const initialCheck = JSON.parse(provenance(["check", "--repo", ".", "--format", "json"], application));
assert.equal(initialCheck.status, "ok", "a freshly initialized project must validate");

const localEngine = join(
  application,
  "node_modules",
  ...engineManifest.name.split("/"),
  "bin",
  binaryName,
);
writeFileSync(join(application, "runtime.mjs"), `
export function startWorkflow() {}
export class WorkflowRunner {
  static constructions = 0;
  constructor() { WorkflowRunner.constructions += 1; }
}
`);
writeFileSync(join(application, "runtime-types.ts"), `
export class WorkflowRunner {}
`);
writeFileSync(join(application, "helpers.ts"), `
import type {
  RequirementDeclaration,
  RuleDeclaration,
  SourceDeclaration,
  SpecAuthoring,
} from "@quality-sh/provenance";

export function guide<const Spec extends string>(
  author: SpecAuthoring<Spec>,
): SourceDeclaration<Spec, "guide"> {
  return author.source("guide").name("Packed SDK guide").document("README.md");
}

export function installed<const Spec extends string>(
  author: SpecAuthoring<Spec>,
  source: SourceDeclaration<Spec, "guide">,
): RequirementDeclaration<Spec, "installed"> {
  return author.requirement("installed")
    .statement("The packed SDK exposes spec-bound TypeScript declarations")
    .description("Exercises emitted fluent metadata types")
    .from(source);
}

export function invocation<
  const Spec extends string,
  const RequirementKey extends string,
>(
  requirement: RequirementDeclaration<Spec, RequirementKey>,
): RuleDeclaration<Spec, "invocation", RequirementKey> {
  return requirement.rule("invocation")
    .statement("A direct Rule handle remains typed across module boundaries");
}
export function bindClass<
  const Spec extends string,
  const Key extends string,
  const RequirementKey extends string | undefined,
>(
  declaration: RuleDeclaration<Spec, Key, RequirementKey>,
  target: abstract new (...args: never[]) => unknown,
): RuleDeclaration<Spec, Key, RequirementKey> {
  return declaration.implementedBy(target);
}
`);
writeFileSync(join(application, "consumer.ts"), `
import { defineSpec } from "@quality-sh/provenance";
import { WorkflowRunner } from "./runtime-types.js";
import { bindClass, guide, installed, invocation as declareInvocation } from "./helpers.js";

const provenance = defineSpec("packed-typescript-consumer");
const packedGuide = guide(provenance);
const packedInstallation = installed(provenance, packedGuide);
export const invocation = bindClass(declareInvocation(packedInstallation), WorkflowRunner);
export const spec = provenance.build(packedInstallation.rules(invocation));

void invocation.verify("packed-consumer", () => undefined);
`);
writeFileSync(join(application, "tsconfig.json"), JSON.stringify({
  compilerOptions: {
    module: "NodeNext",
    moduleResolution: "NodeNext",
    target: "ES2022",
    strict: true,
    noEmit: true,
    skipLibCheck: true,
  },
  include: ["*.ts"],
}));
execFileSync(process.execPath, [
  join(application, "node_modules", "typescript", "bin", "tsc"),
  "-p", join(application, "tsconfig.json"),
], { cwd: application, stdio: "pipe" });
writeFileSync(join(application, "verify.mjs"), `
import { apply, defineSpec, plan, requirement, rule, source } from "@quality-sh/provenance";
import { startWorkflow, WorkflowRunner } from "./runtime.mjs";
const spec = defineSpec("packed-install", ({ requirement }) => {
  const installed = requirement("installed", {
    statement: "The installed SDK invokes its package-supplied engine"
  });
  const invocation = installed.rule("invocation", {
    statement: "Typed SDK operations reach the packaged Rust engine"
  });
  return { installed, invocation };
});
const preview = await plan(spec);
if (preview.created !== 2) throw new Error("plan did not reach the packaged engine");
const result = await apply(spec);
if (result.created !== 2) throw new Error("apply did not reach the packaged engine");
await spec.handles.invocation.verify("packed-install", () => undefined, {
  file: "verify.mjs",
  symbol: "packedInstall"
});

const typedSpec = defineSpec("packed-implemented-by")
  .requirements(
    requirement("typed-implementation")
      .statement("Installed typed specs retain implementation links")
      .from(source("packed-guide").name("Packed guide").document("README.md"))
      .rules(
        rule("typed-start")
          .statement("Installed typed specs resolve imported production implementations")
          .implementedBy(startWorkflow),
        rule("typed-runner")
          .statement("Installed typed specs resolve imported production classes")
          .implementedBy(WorkflowRunner),
      ),
  )
  .build();
const typedResult = await apply(typedSpec);
if (typedResult.implementation_bindings?.some(
  ({ file, symbol }) => file === "runtime.mjs" && symbol === "startWorkflow"
) !== true) {
  throw new Error("implementedBy did not survive the packed install: " + JSON.stringify(typedResult));
}
if (typedResult.implementation_bindings?.some(
  ({ file, symbol }) => file === "runtime.mjs" && symbol === "WorkflowRunner"
) !== true) {
  throw new Error("class implementedBy did not survive the packed install: " + JSON.stringify(typedResult));
}
if (WorkflowRunner.constructions !== 0) {
  throw new Error("implementedBy constructed the packed class target");
}
if (typedResult.resources?.some(({ kind }) => kind === "source") !== true) {
  throw new Error("Requirement.from Source was not collected by build");
}
await typedSpec.requirements["typed-implementation"].rules["typed-start"].verify(
  "packed-direct-rule",
  () => undefined,
  { file: "verify.mjs", symbol: "packedDirectRule" },
);
`);

const { PROVENANCE_BIN: _removed, ...environment } = process.env;
execFileSync(process.execPath, [join(application, "verify.mjs")], {
  cwd: application,
  env: { ...environment, PATH: "" },
  stdio: "pipe",
});
const runs = JSON.parse(execFileSync(localEngine, [
  "sdk", "verification-runs", "--repo", application, "--scope", "default", "--format", "json",
], { encoding: "utf8" }));
assert.equal(runs.length, 2);
assert.deepEqual(runs.map(({ status }) => status), ["passed", "passed"]);

// An install that skips optional dependencies, or a host with no published
// engine, must still answer with Provenance's own guidance. npm carries an
// unrelated package called `provenance`, so a command npm cannot find in the
// project is worse than a clear failure: npx offers to fetch that one instead.
const withoutEngine = join(temporary, "application-without-engine");
mkdirSync(withoutEngine);
writeFileSync(join(withoutEngine, "package.json"), JSON.stringify({
  name: "provenance-clean-install-without-engine",
  private: true,
  version: "1.0.0",
  overrides: { [typescriptManifest.name]: `file:../archives/${typescriptArchive}` },
}));
npm(
  [
    "install",
    "--offline",
    "--cache",
    isolatedCache,
    "--omit=optional",
    "--no-audit",
    "--no-fund",
    join(archiveDirectory, mainArchive),
  ],
  { cwd: withoutEngine },
);
let missingEngine;
try {
  provenance(["init", "--path", ".", "--scope", "default", "--path-prefix", "."], withoutEngine);
} catch (error) {
  missingEngine = error;
}
assert.ok(missingEngine, "init must fail when the install left out the platform engine");
const guidance = `${missingEngine.stdout ?? ""}${missingEngine.stderr ?? ""}`;
assert.match(
  guidance,
  new RegExp(`${engineManifest.name.replace("/", "\\/")} is missing`),
  `the installed bin must name the absent engine package, said: ${guidance}`,
);
assert.doesNotMatch(
  guidance,
  /registry\.npmjs\.org|canceled due to missing packages/,
  `the installed bin must not send the quick start to the registry, said: ${guidance}`,
);

function provenance(args, cwd) {
  const { PROVENANCE_BIN: _removed, ...environment } = process.env;
  return execFileSync(process.execPath, [npxCli, "--no", "provenance", ...args], {
    cwd,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    env: {
      ...environment,
      npm_config_offline: "true",
      npm_config_cache: isolatedCache,
      npm_config_update_notifier: "false",
    },
  });
}

function archiveNamed(archives, expected) {
  const archive = archives.find((name) => name === expected);
  assert.ok(archive, `missing ${expected}; found ${archives.join(", ")}`);
  return archive;
}

function archiveName(manifest) {
  return `${manifest.name.replace(/^@/, "").replace("/", "-")}-${manifest.version}.tgz`;
}

function npm(args, options = {}) {
  execFileSync(process.execPath, [npmCli, ...args], { ...options, stdio: "pipe" });
}

function targetFor(platform, arch) {
  const key = `${platform}-${arch}`;
  const targets = {
    "darwin-arm64": "aarch64-apple-darwin",
    "darwin-x64": "x86_64-apple-darwin",
    "linux-x64": "x86_64-unknown-linux-gnu",
    "win32-x64": "x86_64-pc-windows-msvc",
  };
  const target = targets[key];
  if (target === undefined) throw new Error(`packed install test does not support ${key}`);
  return target;
}
