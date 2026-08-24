import { createHash } from "node:crypto";
import { chmodSync, copyFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { basename, join } from "node:path";
import { fileURLToPath } from "node:url";

// @provenance rule: rule_sdk_install_has_no_binary_fetch
const targets = {
  "aarch64-apple-darwin": {
    name: "@quality-sh/provenance-darwin-arm64",
    os: ["darwin"],
    cpu: ["arm64"],
    binary: "provenance",
  },
  "x86_64-apple-darwin": {
    name: "@quality-sh/provenance-darwin-x64",
    os: ["darwin"],
    cpu: ["x64"],
    binary: "provenance",
  },
  "x86_64-pc-windows-msvc": {
    name: "@quality-sh/provenance-win32-x64-msvc",
    os: ["win32"],
    cpu: ["x64"],
    binary: "provenance.exe",
  },
  "x86_64-unknown-linux-gnu": {
    name: "@quality-sh/provenance-linux-x64-gnu",
    os: ["linux"],
    cpu: ["x64"],
    libc: ["glibc"],
    binary: "provenance",
  },
};

const args = parseArgs(process.argv.slice(2));
const target = targets[required(args, "target")];
if (target === undefined) {
  throw new Error(`unsupported Rust target ${args.target}; expected ${Object.keys(targets).join(", ")}`);
}
const source = required(args, "binary");
const output = required(args, "out");
const version = required(args, "version");
const destination = join(output, "bin", target.binary);
mkdirSync(join(output, "bin"), { recursive: true });
copyFileSync(source, destination);
if (target.os[0] !== "win32") {
  chmodSync(destination, 0o755);
}

const manifest = {
  name: target.name,
  version,
  description: "Platform engine for @quality-sh/provenance",
  license: "BUSL-1.1",
  repository: {
    type: "git",
    url: "git+https://github.com/quality-sh/provenance.git",
  },
  os: target.os,
  cpu: target.cpu,
  ...(target.libc === undefined ? {} : { libc: target.libc }),
  engines: { node: ">=20" },
  preferUnplugged: true,
  files: ["bin", "SHA256SUMS", "README.md", "LICENSE"],
  // A platform package carries the binary and nothing else. The command name
  // stays with @quality-sh/provenance, which every install has, so `npx
  // provenance` behaves the same whether or not this package was installed.
  exports: { "./bin": `./bin/${target.binary}` },
};
writeFileSync(join(output, "package.json"), `${JSON.stringify(manifest, null, 2)}\n`);
copyFileSync(fileURLToPath(new URL("../../../README.md", import.meta.url)), join(output, "README.md"));
copyFileSync(fileURLToPath(new URL("../../../LICENSE", import.meta.url)), join(output, "LICENSE"));

const bytes = readFileSync(destination);
const digest = createHash("sha256").update(bytes).digest("hex");
writeFileSync(join(output, "SHA256SUMS"), `${digest}  bin/${basename(destination)}\n`);

function parseArgs(values) {
  const parsed = {};
  for (let index = 0; index < values.length; index += 2) {
    const flag = values[index];
    const value = values[index + 1];
    if (flag?.startsWith("--") !== true || value === undefined) {
      throw new Error("arguments must be --name value pairs");
    }
    parsed[flag.slice(2)] = value;
  }
  return parsed;
}

function required(values, name) {
  const value = values[name];
  if (value === undefined || value.trim() === "") {
    throw new Error(`--${name} is required`);
  }
  return value;
}
