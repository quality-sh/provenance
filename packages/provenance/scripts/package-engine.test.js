import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
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
