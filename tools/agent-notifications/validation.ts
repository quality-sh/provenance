/** Validates unknown client data before a caller reads its fields. */
export function object(value: unknown): Record<string, unknown> {
  if (!value || typeof value !== "object" || Array.isArray(value)) throw new Error("Expected an object");
  return value as Record<string, unknown>;
}

export function requiredString(value: Record<string, unknown>, name: string): string {
  const field = value[name];
  if (typeof field !== "string" || !field.trim()) throw new Error(`Configuration requires ${name}`);
  return field;
}
