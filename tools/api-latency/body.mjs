// Builds request bodies from a route's request schema. Values come, in this
// order, from the fixed field values (actor names), a seed record of the same
// kind, and the schema itself (const, enum, default, type).

/** Resolve a local `$ref` against the root schema. */
export function resolveRef(root, schema) {
  let current = schema;
  for (let depth = 0; depth < 10 && current?.$ref; depth++) {
    const name = current.$ref.replace('#/$defs/', '');
    current = root?.$defs?.[name];
  }
  return current ?? {};
}

/** The branches of a oneOf or anyOf, or the schema itself. */
function branches(root, schema) {
  const resolved = resolveRef(root, schema);
  const list = resolved.oneOf ?? resolved.anyOf;
  if (!list) {
    return [resolved];
  }
  return list.map(item => resolveRef(root, item));
}

function types(schema) {
  return Array.isArray(schema.type) ? schema.type : schema.type ? [schema.type] : [];
}

function matchesType(schema, value) {
  const allowed = types(schema);
  if (schema.const !== undefined) {
    return value === schema.const;
  }
  if (schema.enum) {
    return schema.enum.includes(value);
  }
  if (!allowed.length) {
    return value !== undefined;
  }
  return allowed.some(type => {
    switch (type) {
      case 'array':
        return Array.isArray(value);
      case 'object':
        return value !== null && typeof value === 'object' && !Array.isArray(value);
      case 'integer':
        return Number.isInteger(value);
      case 'null':
        return value === null;
      default:
        return typeof value === type;
    }
  });
}

/** A value of the schema that uses no seed. */
export function schemaValue(root, schema, context, depth = 0) {
  const options = branches(root, schema);
  const choice = options.find(item => !types(item).includes('null') || types(item).length > 1) ?? options[0];
  if (choice.const !== undefined) {
    return choice.const;
  }
  if (choice.enum?.length) {
    return choice.enum[0];
  }
  if (choice.default !== undefined && choice.default !== null) {
    return choice.default;
  }
  const kinds = types(choice).filter(type => type !== 'null');
  switch (kinds[0]) {
    case 'string':
      return choice.pattern ? context.idText : context.text;
    case 'integer':
    case 'number':
      return choice.minimum ?? 1;
    case 'boolean':
      return false;
    case 'array':
      if ((choice.minItems ?? 0) > 0 && depth < 4) {
        return [schemaValue(root, choice.items ?? {}, context, depth + 1)];
      }
      return [];
    case 'object':
    case undefined:
      if (choice.properties && depth < 4) {
        return objectValue(root, choice, null, context, depth + 1);
      }
      return types(choice).includes('null') ? null : {};
    default:
      return null;
  }
}

function objectValue(root, schema, seed, context, depth) {
  const value = {};
  for (const name of schema.required ?? []) {
    value[name] = fieldValue(root, name, schema.properties?.[name] ?? {}, seed, context, depth);
  }
  return value;
}

function fieldValue(root, name, schema, seed, context, depth) {
  const property = resolveRef(root, schema);
  if (Object.hasOwn(context.fields, name)) {
    const fixed = context.fields[name];
    if (branches(root, property).some(item => matchesType(item, fixed))) {
      return fixed;
    }
    // An object field, for example an actor with an identity type, takes the fixed value as its id.
    const object = branches(root, property).find(item => item.properties?.id);
    if (object) {
      return objectValue(root, object, { id: fixed }, { ...context, fields: {} }, depth + 1);
    }
  }
  const seedValue = seed?.[name];
  if (seedValue !== undefined && branches(root, property).some(item => matchesType(item, seedValue))) {
    return seedValue;
  }
  const reference = context.reference?.(name);
  if (reference !== undefined && reference !== null) {
    return reference;
  }
  return schemaValue(root, property, context, depth);
}

/**
 * The required fields of a request body. `seed` is a record of the same kind
 * or null. `context.fields` holds fixed values by field name, for example
 * the actor and a new record id.
 */
export function requestBody(schema, seed, context) {
  if (!schema) {
    return null;
  }
  const options = branches(schema, schema)
    .map(item => (item === schema ? item : mergeBranch(schema, item)))
    .filter(item => item.properties);
  if (!options.length) {
    return {};
  }
  const best = options
    .map(item => ({ item, score: Object.keys(item.properties).filter(name => seed && name in seed).length }))
    .sort((a, b) => b.score - a.score)[0].item;
  return objectValue(schema, best, seed, context, 0);
}

function mergeBranch(top, branch) {
  return {
    ...branch,
    properties: { ...top.properties, ...branch.properties },
    required: [...new Set([...(top.required ?? []), ...(branch.required ?? [])])],
  };
}

/** The id of a new record: the seed id's prefix, then a unique word. */
export function freshId(seedId, word) {
  const prefix = seedId?.match(/^[a-z0-9]+_/)?.[0] ?? '';
  return `${prefix}${word}`.toLowerCase().replace(/[^a-z0-9_-]/g, '_');
}

const PATCH_FIELDS = ['title', 'name', 'description', 'summary', 'question', 'statement', 'position', 'rationale'];

/** The text field that a PATCH changes, or null when the schema has none. */
export function patchField(schema) {
  const properties = schema?.properties ?? {};
  return PATCH_FIELDS.find(name => {
    const property = resolveRef(schema, properties[name]);
    return name in properties && branches(schema, property).some(item => types(item).includes('string'));
  }) ?? null;
}

/** The first value of `key` in a JSON value, found depth first. */
export function findKey(value, key, depth = 0) {
  if (value === null || typeof value !== 'object' || depth > 6) {
    return undefined;
  }
  if (!Array.isArray(value) && value[key] !== undefined && value[key] !== null) {
    return value[key];
  }
  for (const child of Object.values(value)) {
    const found = findKey(child, key, depth + 1);
    if (found !== undefined) {
      return found;
    }
  }
  return undefined;
}
