import { spawn } from "node:child_process";
import { resolveEnginePath } from "./engine-path.js";

export interface EngineSettings {
  engine?: string;
  repository?: string;
  scope: string;
}

interface EngineInfo {
  engine_version: string;
  protocol_version: number;
  state_schema_version: number;
  repository: string;
}

const SUPPORTED_PROTOCOL_VERSION = 4;
const handshakes = new Map<string, Promise<void>>();

export async function invokeEngine<Result>(
  settings: EngineSettings,
  command: string,
  input?: unknown,
): Promise<Result> {
  const engine = resolveEnginePath(settings.engine);
  await compatibleEngine(settings, engine);
  return invoke<Result>(settings, engine, command, input);
}

// @provenance rule: rule_sdk_protocol_handshake
async function compatibleEngine(settings: EngineSettings, engine: string): Promise<void> {
  let handshake = handshakes.get(engine);
  if (handshake === undefined) {
    handshake = invoke<EngineInfo>(settings, engine, "info").then((info) => {
      if (info.protocol_version !== SUPPORTED_PROTOCOL_VERSION) {
        throw new Error(
          `Provenance engine ${info.engine_version} uses protocol version ${info.protocol_version}; ` +
          `this SDK supports protocol version ${SUPPORTED_PROTOCOL_VERSION}`,
        );
      }
    });
    handshakes.set(engine, handshake);
  }
  await handshake;
}

async function invoke<Result>(
  settings: EngineSettings,
  engine: string,
  command: string,
  input?: unknown,
): Promise<Result> {
  const args = ["sdk", command];
  if (settings.repository !== undefined) {
    args.push("--repo", settings.repository);
  }
  if (command !== "info") {
    args.push("--scope", settings.scope);
  }
  args.push("--format", "json");
  const child = spawn(
    engine,
    args,
    { stdio: ["pipe", "pipe", "pipe"] },
  );
  const stdout: Buffer[] = [];
  const stderr: Buffer[] = [];
  child.stdout.on("data", (chunk: Buffer) => stdout.push(chunk));
  child.stderr.on("data", (chunk: Buffer) => stderr.push(chunk));
  const completed = new Promise<number>((resolve, reject) => {
    child.once("error", reject);
    child.once("close", (code) => resolve(code ?? 1));
  });
  child.stdin.end(input === undefined ? undefined : JSON.stringify(input));
  const code = await completed.catch((cause: unknown) => {
    throw new Error(
      `Provenance engine could not start at ${engine}. Check PROVENANCE_BIN if it is set, ` +
      "or install the development dependency again with optional dependencies enabled.",
      { cause },
    );
  });
  const output = Buffer.concat(stdout).toString("utf8");
  const diagnostics = Buffer.concat(stderr).toString("utf8").trim();
  if (code !== 0) {
    throw new Error(
      `Provenance engine command \`${command}\` failed (${code}): ${diagnostics || output}`,
    );
  }
  try {
    return JSON.parse(output) as Result;
  } catch (error) {
    throw new Error(
      `Provenance engine command \`${command}\` returned invalid JSON`,
      { cause: error },
    );
  }
}
