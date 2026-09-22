/** Shared document traversal and naming helpers for the client templates. */

export function operations(document) {
  return Object.entries(document.paths).flatMap(([path, item]) =>
    ['get', 'post', 'patch'].flatMap(method => item[method] ? [{ path, method, op: item[method] }] : []));
}

export const pascal = value => value.split(/[^a-zA-Z0-9]+/).filter(Boolean)
  .map(part => part[0].toUpperCase() + part.slice(1)).join('');

export const propertyName = parameter => parameter.in === 'header'
  ? parameter.name.toLowerCase().replaceAll('-', '_') : parameter.name;

export const queryVariants = op => op['x-provenance-query-variants'] ?? [];

export const isStringEnum = schema => Array.isArray(schema.enum) && schema.enum.every(value => typeof value === 'string');
