import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const canonicalRepository = {
  type: "git",
  url: "git+https://github.com/quality-sh/provenance.git",
};

const targets = JSON.parse(readFileSync(
  fileURLToPath(new URL("../../../.github/release-targets.json", import.meta.url)),
  "utf8",
));

// @provenance verification: examples
// @provenance rule: rule_sdk_install_has_no_binary_fetch
test("every Rust release target becomes a checksummed npm platform package", () => {
  for (const { target, executable_suffix: executableSuffix, npm: packageData } of targets) {
    const temporary = mkdtempSync(join(tmpdir(), "provenance-engine-package-"));
    const binary = join(temporary, "built-provenance");
    const output = join(temporary, "package");
    writeFileSync(binary, "native engine bytes", { mode: 0o755 });

    execFileSync(process.execPath, [
      fileURLToPath(new URL("./package-engine.js", import.meta.url)),
      "--target", target,
      "--binary", binary,
      "--out", output,
      "--version", "0.1.0",
    ]);

    const manifest = JSON.parse(readFileSync(join(output, "package.json"), "utf8"));
    assert.equal(manifest.name, packageData.name);
    assert.equal(manifest.version, "0.1.0");
    assert.deepEqual(manifest.repository, canonicalRepository);
    assert.deepEqual(manifest.os, packageData.os);
    assert.deepEqual(manifest.cpu, packageData.cpu);
    assert.deepEqual(manifest.libc, packageData.libc);
    assert.equal(
      manifest.preferUnplugged,
      true,
      "native engine packages must ask Plug'n'Play managers to unpack them",
    );

    const binaryName = `provenance${executableSuffix}`;
    const packagedBinary = join(output, "bin", binaryName);
    assert.equal(readFileSync(packagedBinary, "utf8"), "native engine bytes");
    if (!packageData.os.includes("win32")) {
      assert.notEqual(statSync(packagedBinary).mode & 0o111, 0);
    }
    const digest = createHash("sha256").update("native engine bytes").digest("hex");
    assert.equal(
      readFileSync(join(output, "SHA256SUMS"), "utf8"),
      `${digest}  bin/${binaryName}\n`,
    );
  }
});

test("platform package metadata comes from the release target manifest", () => {
  const temporary = mkdtempSync(join(tmpdir(), "provenance-engine-target-"));
  const binary = join(temporary, "built-provenance");
  const output = join(temporary, "package");
  const manifestPath = join(temporary, "release-targets.json");
  writeFileSync(binary, "native engine bytes", { mode: 0o755 });
  writeFileSync(manifestPath, JSON.stringify([{
    target: "test-release-target",
    executable_suffix: "",
    npm: {
      name: "@quality-sh/provenance-test",
      os: ["test-os"],
      cpu: ["test-cpu"],
    },
  }]));

  execFileSync(process.execPath, [
    fileURLToPath(new URL("./package-engine.js", import.meta.url)),
    "--targets", manifestPath,
    "--target", "test-release-target",
    "--binary", binary,
    "--out", output,
    "--version", "0.1.0",
  ]);

  const manifest = JSON.parse(readFileSync(join(output, "package.json"), "utf8"));
  assert.equal(manifest.name, "@quality-sh/provenance-test");
  assert.deepEqual(manifest.os, ["test-os"]);
  assert.deepEqual(manifest.cpu, ["test-cpu"]);
  assert.equal(readFileSync(join(output, "bin", "provenance"), "utf8"), "native engine bytes");
});

test("the TypeScript SDK names the canonical release repository", () => {
  const packageRoot = fileURLToPath(new URL("..", import.meta.url));
  const manifest = JSON.parse(readFileSync(join(packageRoot, "package.json"), "utf8"));
  assert.deepEqual(manifest.repository, canonicalRepository);
});

test("packed dependencies and runtime hosts are generated from release targets", () => {
  const packageRoot = fileURLToPath(new URL("..", import.meta.url));
  const generator = join(packageRoot, "scripts", "generate-release-consumers.js");
  const runtimePath = join(packageRoot, "src", "engine-packages.ts");

  execFileSync(process.execPath, [generator, "--check"]);

  const manifest = JSON.parse(readFileSync(join(packageRoot, "package.json"), "utf8"));
  const expectedPackages = Object.fromEntries(
    targets.map((entry) => [entry.npm.name, manifest.version]),
  );
  assert.deepEqual(manifest.optionalDependencies, expectedPackages);

  const runtime = readFileSync(runtimePath, "utf8");
  for (const entry of targets) {
    const label = [entry.npm.os[0], entry.npm.cpu[0], entry.npm.libc?.[0]]
      .filter((part) => part !== undefined)
      .join("-");
    assert.match(runtime, new RegExp(`${label}.*${entry.npm.name}`));
  }
});

test("consumer generation detects package and runtime drift", () => {
  const temporary = mkdtempSync(join(tmpdir(), "provenance-release-consumers-"));
  const targetsPath = join(temporary, "targets.json");
  const packagePath = join(temporary, "package.json");
  const runtimePath = join(temporary, "engine-packages.ts");
  const generator = fileURLToPath(new URL("./generate-release-consumers.js", import.meta.url));
  writeFileSync(targetsPath, JSON.stringify(targets));
  writeFileSync(packagePath, JSON.stringify({ version: "9.8.7", optionalDependencies: {} }));
  writeFileSync(runtimePath, "stale runtime data\n");

  execFileSync(process.execPath, [
    generator,
    "--targets", targetsPath,
    "--package", packagePath,
    "--runtime", runtimePath,
  ]);
  const generatedPackage = JSON.parse(readFileSync(packagePath, "utf8"));
  assert.deepEqual(
    generatedPackage.optionalDependencies,
    Object.fromEntries(targets.map((entry) => [entry.npm.name, "9.8.7"])),
  );
  assert.match(readFileSync(runtimePath, "utf8"), /linux-x64-glibc/);

  const stalePackage = JSON.parse(readFileSync(packagePath, "utf8"));
  stalePackage.optionalDependencies[targets[0].npm.name] = "0.0.0";
  writeFileSync(packagePath, JSON.stringify(stalePackage));
  let result = spawnSync(process.execPath, [
    generator,
    "--check",
    "--targets", targetsPath,
    "--package", packagePath,
    "--runtime", runtimePath,
  ], { encoding: "utf8" });
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, new RegExp(`stale: ${packagePath.replaceAll("\\", "\\\\")}`));

  execFileSync(process.execPath, [
    generator,
    "--targets", targetsPath,
    "--package", packagePath,
    "--runtime", runtimePath,
  ]);
  writeFileSync(runtimePath, "stale runtime data\n");

  result = spawnSync(process.execPath, [
    generator,
    "--check",
    "--targets", targetsPath,
    "--package", packagePath,
    "--runtime", runtimePath,
  ], { encoding: "utf8" });

  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /generated release consumer is stale/);
});

test("consumer checks accept Windows line endings", () => {
  const temporary = mkdtempSync(join(tmpdir(), "provenance-release-consumers-crlf-"));
  const targetsPath = join(temporary, "targets.json");
  const packagePath = join(temporary, "package.json");
  const runtimePath = join(temporary, "engine-packages.ts");
  const generator = fileURLToPath(new URL("./generate-release-consumers.js", import.meta.url));
  writeFileSync(targetsPath, JSON.stringify(targets));
  writeFileSync(packagePath, JSON.stringify({ version: "9.8.7", optionalDependencies: {} }));
  writeFileSync(runtimePath, "stale runtime data\n");
  const args = [
    generator,
    "--targets", targetsPath,
    "--package", packagePath,
    "--runtime", runtimePath,
  ];

  execFileSync(process.execPath, args);
  for (const path of [packagePath, runtimePath]) {
    writeFileSync(path, readFileSync(path, "utf8").replaceAll("\n", "\r\n"));
  }

  execFileSync(process.execPath, [generator, "--check", ...args.slice(1)]);

  writeFileSync(targetsPath, JSON.stringify(targets.slice(0, -1)));
  const drift = spawnSync(
    process.execPath,
    [generator, "--check", ...args.slice(1)],
    { encoding: "utf8" },
  );
  assert.notEqual(drift.status, 0);
  assert.match(drift.stderr, /generated release consumer is stale/);
});
