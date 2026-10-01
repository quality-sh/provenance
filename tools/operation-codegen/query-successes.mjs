import { operations, queryVariants } from './shared.mjs';

const schemaName = schema => schema?.$ref?.split('/').at(-1);

/**
 * Build each joined query success from its declared variant successes.
 *
 * The Rust exporter joins generic response models before it assigns OpenAPI
 * component names. Equal Rust type names in nested models can then point a
 * joined node field at a different component, such as a response stamp. The
 * query variants already name the exact response schemas. Use those names as
 * the joined union instead of a second structural copy.
 */
export function exactQuerySuccesses(document) {
  const transformed = structuredClone(document);
  const schemas = transformed.components.schemas;
  for (const { op } of operations(transformed)) {
    const variants = queryVariants(op);
    if (variants.length < 2) continue;
    const joined = schemaName(op.responses?.['200']?.content?.['application/json']?.schema);
    if (joined === undefined || schemas[joined] === undefined) continue;
    const successes = variants.map(variant => variant.success);
    if (successes.some(success => schemaName(success) === undefined)) {
      throw new Error(`query-successes: ${op.operationId} has an unnamed success variant`);
    }
    const annotations = Object.fromEntries(Object.entries(schemas[joined])
      .filter(([key]) => key.startsWith('x-') || key === 'title' || key === 'description'));
    schemas[joined] = { ...annotations, anyOf: structuredClone(successes) };
  }
  return transformed;
}
