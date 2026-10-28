// Implementation aid: turns the errors of a renderer response validator into one line per field.

/** A response validator that the renderer bundles. It keeps the errors of its last call. */
export type Validator = ((body: unknown) => boolean) & {
  errors?: {
    instancePath: string;
    keyword: string;
    message?: string;
    params?: { missingProperty?: string };
  }[] | null;
};

// The error names the nearest record id on the path to the field that failed.
function recordAt(body: unknown, path: string): string {
  let node = body as Record<string, unknown> | undefined;
  let id = '(no record)';
  for (const part of path.split('/').slice(1)) {
    if (node && typeof node === 'object' && typeof node.id === 'string') id = node.id;
    node = node?.[part.replaceAll('~1', '/').replaceAll('~0', '~')] as Record<string, unknown> | undefined;
  }
  return node && typeof node === 'object' && typeof node.id === 'string' ? node.id : id;
}

/** The fields that the validator refuses in the body, one line each. An accepted body gives none. */
export function refusedFields(validator: Validator, body: unknown): string[] {
  if (validator(body)) return [];
  // A union reports an error for each branch that failed. Only the deepest fields are reported,
  // so the branch that matched the record type shows its field and the other branches add less noise.
  // A missing property is reported at its parent, so its path gets the property name.
  const messages = new Map<string, Set<string>>();
  for (const error of validator.errors ?? []) {
    if (error.keyword === 'anyOf' || error.keyword === 'oneOf') continue;
    const missing = error.keyword === 'required' ? error.params?.missingProperty : undefined;
    const path = missing === undefined ? error.instancePath : `${error.instancePath}/${missing}`;
    messages.set(path, (messages.get(path) ?? new Set()).add(error.message ?? error.keyword));
  }
  const lines: string[] = [];
  for (const [path, texts] of messages) {
    if ([...messages.keys()].some(other => other.startsWith(`${path}/`))) continue;
    lines.push(`record ${recordAt(body, path)} field ${path || '/'}: ${[...texts].join('; ')}`);
  }
  // A validator can refuse a body without a field error; that is a failure too.
  return lines.length ? lines : ['the validator refused the body and named no field'];
}
