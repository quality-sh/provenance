import { spawnSync } from "node:child_process";
import {
  existsSync,
  readFileSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";

const supportedManagers = new Set(["npm", "pnpm", "yarn", "bun", "deno", "nub"]);

const lockfiles = new Map([
  ["npm", ["package-lock.json", "npm-shrinkwrap.json"]],
  ["pnpm", ["pnpm-lock.yaml"]],
  ["yarn", ["yarn.lock"]],
  ["bun", ["bun.lock", "bun.lockb"]],
  ["deno", ["deno.lock", "deno.json", "deno.jsonc"]],
  ["nub", ["nub.lock"]],
]);

export function parseArguments(args, currentDirectory) {
  let projectDirectory = currentDirectory;
  let packageManager;
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === "--path") {
      projectDirectory = resolve(currentDirectory, requiredValue(args, ++index, argument));
    } else if (argument === "--package-manager") {
      packageManager = requiredValue(args, ++index, argument);
    } else {
      throw new Error(`Unknown argument '${argument}'.`);
    }
  }
  return { projectDirectory, packageManager };
}

function requiredValue(args, index, option) {
  const value = args[index];
  if (value === undefined || value.startsWith("--")) {
    throw new Error(`${option} requires a value.`);
  }
  return value;
}

export function initializeProject({
  projectDirectory,
  packageVersion,
  packageSpec,
  enginePath,
  engineArguments = [],
  packageManager,
  userAgent = process.env.npm_config_user_agent,
  execute,
}) {
  const directory = resolve(projectDirectory);
  const selectedManager = selectPackageManager(directory, packageManager, userAgent);
  const run = execute ?? ((invocation) => executeCommand(invocation, directory));

  runChecked(
    run,
    installInvocation(
      selectedManager,
      packageSpec ?? `@quality-sh/provenance@${packageVersion}`,
    ),
    "Provenance installation",
  );
  runChecked(run, {
    command: enginePath,
    args: [
      ...engineArguments,
      "init",
      "--path",
      directory,
      "--scope",
      "default",
      "--path-prefix",
      ".",
    ],
    capture: false,
  }, "Provenance initialization");
  const check = runChecked(run, {
    command: enginePath,
    args: [...engineArguments, "check", "--repo", directory, "--format", "json"],
    capture: true,
  }, "Provenance validation");
  ensureValidProject(check.stdout);
  ensureCacheIgnored(directory);

  return { packageManager: selectedManager };
}

function selectPackageManager(directory, requested, userAgent) {
  if (requested !== undefined) {
    return ensureSupportedManager(requested);
  }

  const declared = declaredPackageManager(directory);
  if (declared !== undefined) {
    return ensureSupportedManager(declared);
  }

  const found = [...lockfiles]
    .filter(([, names]) => names.some((name) => existsSync(join(directory, name))))
    .map(([manager]) => manager);
  if (found.length > 1) {
    throw new Error(
      `More than one package manager is present (${found.join(", ")}). ` +
      "Pass --package-manager with one of: npm, pnpm, yarn, bun, deno, nub.",
    );
  }
  if (found.length === 1) {
    return found[0];
  }

  const invokedBy = userAgent?.match(/^([^/\s]+)\//)?.[1];
  return supportedManagers.has(invokedBy) ? invokedBy : "npm";
}

function declaredPackageManager(directory) {
  const manifestPath = join(directory, "package.json");
  if (!existsSync(manifestPath)) {
    return undefined;
  }
  const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  if (typeof manifest.packageManager !== "string") {
    return undefined;
  }
  return manifest.packageManager.match(/^([^@]+)@/)?.[1] ?? manifest.packageManager;
}

function ensureSupportedManager(manager) {
  if (!supportedManagers.has(manager)) {
    throw new Error(
      `Unsupported package manager '${manager}'. ` +
      "Choose one of: npm, pnpm, yarn, bun, deno, nub.",
    );
  }
  return manager;
}

function installInvocation(manager, packageSpec) {
  switch (manager) {
    case "npm":
      return command("npm", ["install", "--save-dev", "--save-exact", packageSpec]);
    case "pnpm":
      return command("pnpm", ["add", "--save-dev", "--save-exact", packageSpec]);
    case "yarn":
      return command("yarn", ["add", "--dev", "--exact", packageSpec]);
    case "bun":
      return command("bun", ["add", "--dev", "--exact", packageSpec]);
    case "deno":
      return command("deno", [
        "add",
        "--dev",
        "--package-json",
        "--minimum-dependency-age=0",
        packageSpec.startsWith("@quality-sh/") ? `npm:${packageSpec}` : packageSpec,
      ]);
    case "nub":
      return command("nub", [
        "add",
        "-D",
        "-E",
        "--allow-low-downloads",
        "--minimum-release-age=0",
        packageSpec,
      ]);
  }
}

function command(executable, args) {
  return { command: executable, args, capture: false };
}

function runChecked(run, invocation, operation) {
  const result = run(invocation);
  if (result.status !== 0) {
    throw new Error(`${operation} failed with exit code ${result.status ?? "unknown"}.`);
  }
  return result;
}

function ensureValidProject(stdout) {
  let result;
  try {
    result = JSON.parse(stdout);
  } catch {
    throw new Error("The freshly initialized project did not validate: invalid check output.");
  }
  if (result.status !== "ok") {
    throw new Error("The freshly initialized project did not validate.");
  }
}

function ensureCacheIgnored(directory) {
  const ignorePath = join(directory, ".gitignore");
  const current = existsSync(ignorePath) ? readFileSync(ignorePath, "utf8") : "";
  if (current.split(/\r?\n/).includes(".provenance/cache/")) {
    return;
  }
  const separator = current.length > 0 && !current.endsWith("\n") ? "\n" : "";
  writeFileSync(ignorePath, `${current}${separator}.provenance/cache/\n`);
}

function executeCommand({ command, args, capture }, directory) {
  const result = spawnSync(hostExecutable(command), args, {
    cwd: directory,
    encoding: "utf8",
    stdio: capture ? ["ignore", "pipe", "inherit"] : "inherit",
  });
  if (result.error !== undefined) {
    throw result.error;
  }
  return { status: result.status, stdout: result.stdout ?? "" };
}

function hostExecutable(command) {
  if (process.platform !== "win32") {
    return command;
  }
  return ["npm", "pnpm", "yarn", "nub"].includes(command) ? `${command}.cmd` : command;
}
