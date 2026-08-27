import { spawnSync } from "node:child_process";
import {
  existsSync,
  readFileSync,
} from "node:fs";
import { createRequire } from "node:module";
import { dirname, join, resolve } from "node:path";

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
  let steOnboarding = "interactive";
  for (let index = 0; index < args.length; index += 1) {
    const argument = args[index];
    if (argument === "--path") {
      projectDirectory = resolve(currentDirectory, requiredValue(args, ++index, argument));
    } else if (argument === "--package-manager") {
      packageManager = requiredValue(args, ++index, argument);
    } else if (argument === "--ste-onboarding") {
      steOnboarding = requiredValue(args, ++index, argument);
      if (!new Set(["agent", "interactive"]).has(steOnboarding)) {
        throw new Error("Unsupported STE onboarding mode. Choose one of: agent, interactive.");
      }
    } else {
      throw new Error(`Unknown argument '${argument}'.`);
    }
  }
  return { projectDirectory, packageManager, steOnboarding };
}

function requiredValue(args, index, option) {
  const value = args[index];
  if (value === undefined || value.startsWith("--")) {
    throw new Error(`${option} requires a value.`);
  }
  return value;
}

// @provenance rule: rule_typescript_initializer_installs_dev_dependency
// @provenance rule: rule_typescript_initializer_validates_project
export function initializeProject({
  projectDirectory,
  packageVersion,
  packageSpec,
  enginePath,
  engineArguments = [],
  resolveEngine = installedEngineCommand,
  packageManager,
  steOnboarding = "interactive",
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
  const engine = enginePath === undefined
    ? resolveEngine(directory)
    : { command: enginePath, args: engineArguments };
  const initArgs = [
    ...engine.args,
    "init",
    "--path",
    directory,
    "--scope",
    "default",
    "--path-prefix",
    ".",
  ];
  const help = run({
    command: engine.command,
    args: [...engine.args, "init", "--help"],
    capture: true,
  });
  if (supportsSteOnboarding(help)) {
    initArgs.push(
      "--ste-onboarding",
      steOnboarding,
      "--invocation-channel",
      "typescript",
      "--package-manager",
      selectedManager,
    );
  }
  runChecked(run, {
    command: engine.command,
    args: initArgs,
    capture: false,
  }, "Provenance initialization");
  return { packageManager: selectedManager };
}

function supportsSteOnboarding(help) {
  // The visible onboarding flag and its hidden invocation metadata flags were
  // introduced as one init capability. Older engines advertise none of them.
  return help.status === 0 && help.stdout.includes("--ste-onboarding");
}

function installedEngineCommand(directory) {
  const pnpPath = join(directory, ".pnp.cjs");
  const usesPnp = existsSync(pnpPath);
  const sdkEntry = usesPnp
    ? createRequire(pnpPath)(pnpPath).resolveRequest(
      "@quality-sh/provenance",
      join(directory, "package.json"),
    )
    : createRequire(join(directory, "package.json"))
      .resolve("@quality-sh/provenance");
  const sdkRoot = dirname(dirname(sdkEntry));
  return {
    command: usesPnp ? "yarn" : process.execPath,
    args: [
      ...(usesPnp ? ["node"] : []),
      join(sdkRoot, "bin", "provenance.mjs"),
    ],
  };
}

// @provenance rule: rule_typescript_initializer_selects_package_manager
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
        "--save-exact",
        "--package-json",
        packageSpec.startsWith("@quality-sh/") ? `npm:${packageSpec}` : packageSpec,
      ], { NPM_CONFIG_MIN_RELEASE_AGE: "0" });
    case "nub":
      return command("nub", [
        "add",
        "-D",
        "-E",
        "--allow-low-downloads",
        "--minimum-release-age-exclude",
        "@quality-sh/provenance*",
        packageSpec,
      ]);
  }
}

function command(executable, args, environment) {
  return {
    command: executable,
    args,
    capture: false,
    ...(environment === undefined ? {} : { environment }),
  };
}

function runChecked(run, invocation, operation) {
  const result = run(invocation);
  if (result.status !== 0) {
    throw new Error(`${operation} failed with exit code ${result.status ?? "unknown"}.`);
  }
  return result;
}

function executeCommand({ command, args, capture, environment }, directory) {
  const invocation = hostInvocation({ command, args, capture, environment });
  const result = spawnSync(invocation.command, invocation.args, {
    cwd: directory,
    encoding: "utf8",
    env: environment === undefined ? undefined : { ...process.env, ...environment },
    stdio: capture ? ["ignore", "pipe", "inherit"] : "inherit",
  });
  if (result.error !== undefined) {
    throw result.error;
  }
  return { status: result.status, stdout: result.stdout ?? "" };
}

export function hostInvocation(
  invocation,
  platform = process.platform,
  commandInterpreter = process.env.ComSpec ?? "cmd.exe",
) {
  if (platform !== "win32" || !supportedManagers.has(invocation.command)) {
    return invocation;
  }
  return {
    ...invocation,
    command: commandInterpreter,
    args: ["/d", "/s", "/c", invocation.command, ...invocation.args],
  };
}
