#!/usr/bin/env node
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

import { initializeProject, parseArguments } from "../src/initializer.mjs";

if (process.argv.slice(2).some((argument) => argument === "--help" || argument === "-h")) {
  process.stdout.write(`Usage: create-provenance [options]

Install Provenance as a development dependency and initialize the project.

Options:
  --path <path>                 Target project (default: current directory)
  --package-manager <manager>  npm, pnpm, yarn, bun, deno, or nub
  -h, --help                    Show this help
`);
  process.exit(0);
}

try {
  const manifest = JSON.parse(
    readFileSync(fileURLToPath(new URL("../package.json", import.meta.url)), "utf8"),
  );
  const options = parseArguments(process.argv.slice(2), process.cwd());
  const engine = engineCommand();
  const result = initializeProject({
    ...options,
    packageVersion: manifest.version,
    packageSpec: process.env.PROVENANCE_PACKAGE_SPEC,
    enginePath: engine.command,
    engineArguments: engine.args,
  });
  process.stdout.write(
    `Provenance is ready in ${options.projectDirectory} with ${result.packageManager}.\n`,
  );
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exit(1);
}

function engineCommand() {
  if (process.env.PROVENANCE_BIN !== undefined) {
    return { command: process.env.PROVENANCE_BIN, args: [] };
  }
  const require = createRequire(import.meta.url);
  const sdkEntry = require.resolve("@quality-sh/provenance");
  const sdkRoot = dirname(dirname(sdkEntry));
  return {
    command: process.execPath,
    args: [join(sdkRoot, "bin", "provenance.mjs")],
  };
}
