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

const targets = [
  ["aarch64-apple-darwin", "@quality-sh/provenance-darwin-arm64", "darwin", "arm64", undefined],
  ["x86_64-apple-darwin", "@quality-sh/provenance-darwin-x64", "darwin", "x64", undefined],
  ["x86_64-pc-windows-msvc", "@quality-sh/provenance-win32-x64-msvc", "win32", "x64", undefined],
  ["x86_64-unknown-linux-gnu", "@quality-sh/provenance-linux-x64-gnu", "linux", "x64", "glibc"],
];

// @provenance verification: examples
// @provenance rule: rule_sdk_install_has_no_binary_fetch
test("every Rust release target becomes a checksummed npm platform package", () => {
  for (const [target, name, os, cpu, libc] of targets) {
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
    assert.equal(manifest.name, name);
    assert.equal(manifest.version, "0.1.0");
    assert.deepEqual(manifest.repository, canonicalRepository);
    assert.deepEqual(manifest.os, [os]);
    assert.deepEqual(manifest.cpu, [cpu]);
    assert.deepEqual(manifest.libc, libc === undefined ? undefined : [libc]);

    const binaryName = os === "win32" ? "provenance.exe" : "provenance";
    const packagedBinary = join(output, "bin", binaryName);
    assert.equal(readFileSync(packagedBinary, "utf8"), "native engine bytes");
    if (os !== "win32") assert.notEqual(statSync(packagedBinary).mode & 0o111, 0);
    const digest = createHash("sha256").update("native engine bytes").digest("hex");
    assert.equal(
      readFileSync(join(output, "SHA256SUMS"), "utf8"),
      `${digest}  bin/${binaryName}\n`,
    );
  }
});

test("the TypeScript SDK names the canonical release repository", () => {
  const packageRoot = fileURLToPath(new URL("..", import.meta.url));
  const manifest = JSON.parse(readFileSync(join(packageRoot, "package.json"), "utf8"));
  assert.deepEqual(manifest.repository, canonicalRepository);
});
