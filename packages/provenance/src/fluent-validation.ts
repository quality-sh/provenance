export function uniqueByKey<T extends { readonly key: string }>(
  kind: string,
  values: readonly T[],
): T[] {
  const byKey = new Map<string, T>();
  for (const value of values) {
    const existing = byKey.get(value.key);
    if (existing !== undefined && existing !== value) {
      throw new Error(`distinct ${kind} declarations use key \`${value.key}\``);
    }
    byKey.set(value.key, value);
  }
  return [...byKey.values()];
}

export function appendByIdentity<T>(existing: readonly T[], added: readonly T[]): T[] {
  const result = [...existing];
  for (const value of added) {
    if (!result.includes(value)) result.push(value);
  }
  return result;
}

export function requireText(
  field: string,
  value: string | undefined,
): asserts value is string {
  if (value === undefined || value.trim() === "") {
    throw new Error(`${field} must not be empty`);
  }
}
