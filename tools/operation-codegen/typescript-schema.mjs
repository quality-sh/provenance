// OpenAPI 3.1 gives $ref siblings their normal conjunctive meaning. The pinned
// TypeScript generator needs that conjunction stated as allOf explicitly.
export function typescriptSchema(document) {
  const copy = structuredClone(document);
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
