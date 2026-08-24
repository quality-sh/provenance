import assert from "node:assert/strict";
import test from "node:test";

import {
  enginePackageFor,
  resolveEnginePath,
  type EngineHost,
} from "./engine-path.js";

const supported: ReadonlyArray<[EngineHost, string]> = [
  [{ platform: "darwin", arch: "arm64" }, "@quality-sh/provenance-darwin-arm64"],
  [{ platform: "darwin", arch: "x64" }, "@quality-sh/provenance-darwin-x64"],
  [{ platform: "win32", arch: "x64" }, "@quality-sh/provenance-win32-x64-msvc"],
  [{ platform: "linux", arch: "x64", libc: "glibc" }, "@quality-sh/provenance-linux-x64-gnu"],
];

test("each supported host selects one platform engine package", () => {
  for (const [host, packageName] of supported) {
    assert.equal(enginePackageFor(host), packageName);
  }
});

test("unsupported hosts fail with the host and supported targets", () => {
  assert.throws(
    () => enginePackageFor({ platform: "linux", arch: "arm64", libc: "musl" }),
    /linux-arm64-musl.*Supported targets/s,
  );
});

test("an explicit binary bypasses platform package resolution", () => {
  let resolved = false;
  const path = resolveEnginePath("/tmp/custom-provenance", {
    host: { platform: "linux", arch: "x64", libc: "glibc" },
    resolvePackage: () => {
      resolved = true;
      return "/unused";
    },
  });

  assert.equal(path, "/tmp/custom-provenance");
  assert.equal(resolved, false);
});

// @provenance verification: examples
// @provenance rule: rule_sdk_package_supplies_engine
test("the packaged engine path comes from the selected optional dependency", () => {
  const requested: string[] = [];
  const path = resolveEnginePath(undefined, {
    host: { platform: "darwin", arch: "arm64" },
    resolvePackage: (specifier) => {
      requested.push(specifier);
      return "/project/node_modules/@quality-sh/provenance-darwin-arm64/bin/provenance";
    },
  });

  assert.equal(path, "/project/node_modules/@quality-sh/provenance-darwin-arm64/bin/provenance");
  assert.deepEqual(requested, ["@quality-sh/provenance-darwin-arm64/bin"]);
});

test("a missing optional package explains how to repair the install", () => {
  assert.throws(
    () => resolveEnginePath(undefined, {
      host: { platform: "linux", arch: "x64", libc: "glibc" },
      resolvePackage: () => {
        throw new Error("not found");
      },
    }),
    /optional engine package.*install the development dependency again.*optional dependencies/si,
  );
});
