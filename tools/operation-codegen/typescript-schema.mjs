// Native record types carry optional fields; wire validators enforce the
// marked cross-field condition. Project only that condition out of client types.
export function clientTypeSchema(document) {
  const copy = structuredClone(document);
  function normalize(value) {
    if (Array.isArray(value)) return value.map(normalize);
    if (value === null || typeof value !== 'object') return value;
    const entries = Object.fromEntries(Object.entries(value).map(([key, child]) => [key, normalize(child)]));
    if (entries['x-provenance-validation-only-any-of'] === true) {
      delete entries['x-provenance-validation-only-any-of'];
      delete entries.anyOf;
    }
    return entries;
  }
  copy.components.schemas = normalize(copy.components.schemas);
  return copy;
}

// OpenAPI 3.1 gives $ref siblings their normal conjunctive meaning. The pinned
// TypeScript generator needs that conjunction stated as allOf explicitly.
export function typescriptSchema(document) {
  const copy = clientTypeSchema(document);
  function normalize(value) {
    if (Array.isArray(value)) return value.map(normalize);
    if (value === null || typeof value !== 'object') return value;
    const entries = Object.fromEntries(Object.entries(value).map(([key, child]) => [key, normalize(child)]));
    if (typeof entries.$ref === 'string' && Object.keys(entries).length > 1) {
      const { $ref, ...siblings } = entries;
      const { allOf = [], ...constraints } = siblings;
      return { ...constraints, allOf: [{ $ref }, ...allOf] };
    }
    return entries;
  }
  copy.components.schemas = normalize(copy.components.schemas);
  return copy;
}
