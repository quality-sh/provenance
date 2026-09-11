interface PolicyClient {
  read(): Promise<unknown>;
  write(): Promise<unknown>;
  unresolvedWrites?: () => ReadonlyArray<unknown>;
}

export function clientPolicyTests(
  label: string,
  connect: (options: { baseUrl: string; bearer?: string; fetch?: typeof fetch }) => Promise<PolicyClient>,
  protocolVersion: number,
  maxResponseBytes: number,
): void;
