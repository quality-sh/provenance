#!/usr/bin/env node
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { initializeProject, parseArguments } from "../src/initializer.js";

if (process.argv.slice(2).some((argument) => argument === "--help" || argument === "-h")) {
  process.stdout.write(`Usage: create-provenance [options]

Install Provenance as a development dependency and initialize the project.

Options:
  --path <path>                 Target project (default: current directory)
  --package-manager <manager>  npm, pnpm, yarn, bun, deno, or nub
  --ste-onboarding <mode>       interactive (default) or agent
  -h, --help                    Show this help
`);
  process.exit(0);
}

try {
  const manifest = JSON.parse(
    readFileSync(fileURLToPath(new URL("../package.json", import.meta.url)), "utf8"),
  );
  const options = parseArguments(process.argv.slice(2), process.cwd());
  const result = initializeProject({
    ...options,
    packageVersion: manifest.version,
    packageSpec: process.env.PROVENANCE_PACKAGE_SPEC,
    enginePath: process.env.PROVENANCE_BIN,
  });
  process.stdout.write(
    `Provenance is ready in ${options.projectDirectory} with ${result.packageManager}.\n`,
  );
} catch (error) {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exit(1);
}
