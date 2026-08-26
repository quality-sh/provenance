import { createRequire } from "node:module";

import { enginePackages } from "./engine-packages.js";

export interface EngineHost {
  platform: NodeJS.Platform;
  arch: string;
  libc?: "glibc" | "musl";
}

interface ResolutionOptions {
  host?: EngineHost;
  resolvePackage?: (specifier: string) => string;
}

const require = createRequire(import.meta.url);

export function enginePackageFor(host: EngineHost): string {
  const label = hostLabel(host);
  const packageName = enginePackages.get(label);
  if (packageName === undefined) {
    throw new Error(
      `Provenance has no engine for ${label}. Supported targets: ${[...enginePackages.keys()].join(", ")}`,
    );
  }
  return packageName;
}

// @provenance rule: rule_sdk_package_supplies_engine
export function resolveEnginePath(
  override?: string,
  options: ResolutionOptions = {},
): string {
  if (override !== undefined) {
    return override;
  }
  const host = options.host ?? runtimeHost();
  const packageName = enginePackageFor(host);
  const resolvePackage = options.resolvePackage ?? require.resolve;
  try {
    return resolvePackage(`${packageName}/bin`);
  } catch (cause) {
    throw new Error(
      `The optional engine package ${packageName} is missing. Install the development dependency ` +
      `again with optional dependencies enabled for ${hostLabel(host)}.`,
      { cause },
    );
  }
}

function runtimeHost(): EngineHost {
  return {
    platform: process.platform,
    arch: process.arch,
    libc: process.platform === "linux" ? runtimeLibc() : undefined,
  };
}

function runtimeLibc(): "glibc" | "musl" {
  const report = process.report?.getReport();
  if (typeof report === "object" && report !== null) {
    const header = (report as { header: Record<string, unknown> }).header;
    if (header.glibcVersionRuntime !== undefined) {
      return "glibc";
    }
  }
  return "musl";
}

function hostLabel(host: EngineHost): string {
  return [host.platform, host.arch, host.platform === "linux" ? host.libc ?? "unknown-libc" : undefined]
    .filter((part) => part !== undefined)
    .join("-");
}
