interface PolicyClient {
  read(): Promise<unknown>;
  write(): Promise<unknown>;
}

export function clientPolicyTests(
  label: string,
  connect: (options: { baseUrl: string; bearer?: string; fetch?: typeof fetch; repository?: string; scope?: string }) => Promise<PolicyClient>,
  protocolVersion: number,
  maxResponseBytes: number,
): void;
