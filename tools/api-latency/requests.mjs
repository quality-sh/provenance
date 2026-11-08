// Finds the input for each operation from the state of the scratch clone.
// Path parameters come from the list route above them, writes get new
// records to act on, and a route whose input cannot be found is reported
// with the reason.
import { fillPath, pathParameters } from './catalog.mjs';
import { findKey, freshId, patchField, requestBody, resolveRef } from './body.mjs';

export class Unresolved extends Error {}

const CANDIDATES = 25;
const READ_ATTEMPTS = 60;

/**
 * Writes that need another write on the same record before them. A create
 * submits the new record, so a submit first withdraws that submission.
 */
const BEFORE = { release: 'claim', submit: 'submissions/{proposal_id}/withdraw' };

/** Path parameters that a write on a sibling or other route creates. */
const PRODUCERS = { proposal_id: 'submit', run_id: '/verification-runs/begin-verification' };

function segments(path) {
  return path.split('/').filter(Boolean);
}

function template(parts) {
  return `/${parts.join('/')}`;
}

/** The candidate values of an item in a list answer for parameter `name`. */
export function itemValue(item, name) {
  return item?.[name] ?? findKey(item, name) ?? item?.id ?? null;
}

/** The top-level collection that holds the records a parameter names. */
export function collectionFor(name, collections) {
  const stem = name.replace(/_id$/, '');
  return collections.find(path => {
    const last = segments(path).at(-1);
    return last === `${stem}s` || last.endsWith(`-${stem}s`);
  }) ?? null;
}

export class Planner {
  constructor({ operations, context }) {
    this.operations = operations;
    this.context = context;
    this.counter = 0;
    this.added = [];
    this.made = new Map();
    this.empty = new Set();
    this.lists = new Set(operations.filter(op => op.method === 'get' && !op.selector && !op.parameters.some(p => p.required && p.location === 'query')).map(op => op.path));
    this.collections = [...this.lists].filter(path => segments(path).length === 1);
  }

  find(method, path) {
    return this.operations.find(op => op.method === method && op.path === path && !op.selector) ?? null;
  }

  async list(call, path, query = {}) {
    const result = await call({ method: 'get', path, query: { limit: CANDIDATES, ...query } });
    return result.ok ? (result.data?.items ?? []) : [];
  }

  /** The query string of an operation: the selector and each required parameter. */
  query(op) {
    const query = {};
    for (const parameter of op.parameters) {
      if (parameter.location !== 'query' || !(parameter.required || parameter.name === 'query')) {
        continue;
      }
      if (parameter.schema?.const !== undefined) {
        query[parameter.name] = parameter.schema.const;
      } else if (this.context.query[parameter.name] !== undefined) {
        query[parameter.name] = this.context.query[parameter.name];
      } else {
        throw new Unresolved(`the required query parameter ${parameter.name} has no value source`);
      }
    }
    return query;
  }

  /** Candidate values for path parameter `index` of `path`, given the earlier values. */
  async candidates(call, op, names, index, values) {
    const name = names[index];
    const schema = op.parameters.find(item => item.name === name)?.schema;
    if (schema?.enum?.length) {
      return schema.enum;
    }
    const parts = segments(op.path);
    const at = parts.indexOf(`{${name}}`);
    const prefix = template(parts.slice(0, at));
    const made = index === 0 ? (this.made.get(`/${parts[0]}`) ?? []) : [];
    const owners = index === 0 && names[1] ? [...made, ...await this.owners(call, names[1])] : made;
    let listPath = this.lists.has(prefix) ? prefix : null;
    if (!listPath) {
      listPath = collectionFor(name, this.collections);
    }
    if (!listPath) {
      throw new Unresolved(`no list route gives values for path parameter ${name}`);
    }
    const query = segments(listPath).length === 1 ? { exclude_terminal: true } : {};
    let items = await this.list(call, fillPath(listPath, values), query);
    let label = listPath;
    if (listPath !== prefix && index > 0) {
      // A top-level index lists the items of every owner: keep the items of this owner.
      const known = new Set(Object.values(values));
      const all = items.length;
      items = items.filter(item => !findKey(item, 'node_id') || known.has(findKey(item, 'node_id')));
      if (all && !items.length) {
        label = `${listPath} (for a record of this kind)`;
      }
    }
    if (!items.length && !owners.length) {
      this.empty.add(label);
    }
    return [...new Set([...owners, ...items.map(item => itemValue(item, name)).filter(Boolean)])];
  }

  /** The owner record ids of the items in the top-level collection that a parameter names. */
  async owners(call, name) {
    const collection = collectionFor(name, this.collections);
    if (!collection) {
      return [];
    }
    const items = await this.list(call, collection, { exclude_terminal: true });
    return [...new Set(items.map(item => findKey(item, 'node_id')).filter(Boolean))];
  }

  async searchPath(call, op, names, index, values, query) {
    if (this.budget-- <= 0) {
      return null;
    }
    if (index === names.length) {
      const request = { method: 'get', path: fillPath(op.path, values), query };
      const result = await call(request);
      return result.ok ? request : null;
    }
    for (const value of await this.candidates(call, op, names, index, values)) {
      const found = await this.searchPath(call, op, names, index + 1, { ...values, [names[index]]: value }, query);
      if (found) {
        return found;
      }
    }
    return null;
  }

  /** A read request that the scratch clone answers with success. */
  async read(call, op) {
    const query = this.query(op);
    this.budget = READ_ATTEMPTS;
    this.empty = new Set();
    const names = pathParameters(op.path);
    let request = await this.searchPath(call, op, names, 0, {}, query);
    if (!request && this.empty.size && names.length) {
      request = await this.readOnNewRecords(call, op, names, query);
    }
    if (!request) {
      const empty = [...this.empty];
      throw new Unresolved(empty.length
        ? `the state has no record for this route: ${empty.join(' and ')} gave no items`
        : `no value of the path parameters gave a successful answer in ${READ_ATTEMPTS} tries`);
    }
    return request;
  }

  /**
   * Make the records that a read route needs when the state has none, with
   * the catalog's own writes. The run reports each record made this way.
   */
  async readOnNewRecords(call, op, names, query) {
    try {
      await this.loadSamples(call);
      const collection = `/${segments(op.path)[0]}`;
      const first = { [names[0]]: await this.newRecord(call, collection, 'plan') };
      this.budget = READ_ATTEMPTS;
      let request = names.length > 1 ? await this.searchPath(call, op, names, 1, first, query) : null;
      if (!request) {
        request = { method: 'get', path: fillPath(op.path, await this.writeValues(call, op, 'plan', first)), query };
        if (!(await call(request)).ok) {
          return null;
        }
      }
      this.added.push({ call: op.label, path: request.path });
      return request;
    } catch (error) {
      if (!(error instanceof Unresolved)) {
        throw error;
      }
      this.addFailure = error.message;
    }
    return null;
  }

  bodyContext(id) {
    return {
      text: this.context.text,
      idText: this.context.idText,
      fields: { ...this.context.fields, ...(id ? { id } : {}), supersedes: [] },
      reference: name => this.reference(name),
    };
  }

  /** Read the first record id of each top-level collection, for reference fields. */
  async loadSamples(call) {
    if (this.samples) {
      return;
    }
    this.samples = {};
    for (const collection of this.collections) {
      const [first] = await this.list(call, collection, { exclude_terminal: true, limit: 1 });
      if (first?.id) {
        this.samples[collection] = first.id;
      }
    }
  }

  /** A record id for a field named `<kind>_id`, or a list of one for `<kind>_ids`. */
  reference(name) {
    const many = name.endsWith('_ids');
    const single = many ? `${name.slice(0, -4)}_id` : name;
    if (!single.endsWith('_id')) {
      return undefined;
    }
    const id = this.samples?.[collectionFor(single, this.collections)];
    if (!id) {
      return undefined;
    }
    return many ? [id] : id;
  }

  /** A record of a collection: the first one that is not terminal. */
  async seed(call, collection) {
    const [first] = await this.list(call, collection, { exclude_terminal: true, limit: 1 });
    if (!first) {
      return null;
    }
    const member = await call({ method: 'get', path: `${collection}/${encodeURIComponent(first.id)}` });
    return member.ok ? member.data : first;
  }

  /** The create request of a collection, with a new record id. */
  /** The route that makes a record of a collection: POST on it, or one POST below it with no parameters. */
  createRoute(collection) {
    return this.find('post', collection)
      ?? this.operations.find(op => op.method === 'post' && op.write && op.path.startsWith(`${collection}/`) && !op.path.includes('{') && segments(op.path).length === 2)
      ?? null;
  }

  async createRequest(call, collection, way) {
    await this.loadSamples(call);
    const create = this.createRoute(collection);
    if (!create) {
      throw new Unresolved(`the collection ${collection} has no create route`);
    }
    const seed = await this.seed(call, collection);
    const id = freshId(seed?.id, `bench_${way.toLowerCase()}_${process.pid}_${this.counter++}`);
    return { method: 'post', path: create.path, query: {}, headers: {}, body: requestBody(create.requestSchema, seed, this.bodyContext(id)) };
  }

  /** Make a new record of a collection and give its id. */
  async newRecord(call, collection, way) {
    const request = await this.createRequest(call, collection, way);
    const result = await call(request);
    if (!result.ok) {
      throw new Unresolved(`the create of a record in ${collection} failed: ${describe(result.error)}`);
    }
    const id = request.body?.id ?? findKey(result.data, 'id');
    if (way === 'plan') {
      this.made.set(collection, [...(this.made.get(collection) ?? []), id]);
    }
    return id;
  }

  async etag(call, path) {
    const result = await call({ method: 'get', path });
    const version = result.ok ? findKey(result.data, 'version') : undefined;
    const etag = result.ok ? (result.etag ?? findKey(result.data, 'etag') ?? (version === undefined ? undefined : String(version))) : undefined;
    if (!etag) {
      throw new Unresolved(`the read of ${path} gave no ETag`);
    }
    return etag;
  }

  /** Run another write on the record and give the value it creates for `name`. */
  async produce(call, op, name, values, way) {
    const producer = PRODUCERS[name];
    const parts = segments(op.path);
    const at = parts.indexOf(`{${name}}`);
    const owner = this.memberPath({ path: template(parts.slice(0, at)) }, values);
    if (owner) {
      const record = await call({ method: 'get', path: owner });
      const pending = record.ok ? pendingValue(record.data, name) : undefined;
      if (pending) {
        return pending;
      }
    }
    let route = null;
    if (producer?.startsWith('/')) {
      route = this.find('post', producer);
    } else if (producer) {
      route = this.find('post', template([...parts.slice(0, Math.max(at - 1, 0)), producer]));
    }
    route ??= this.find('post', template(parts.slice(0, at)));
    if (!route) {
      throw new Unresolved(`no write creates path parameter ${name}`);
    }
    const request = await this.writeRequest(call, route, values, way);
    const result = await call(request);
    let value = result.ok ? (findKey(result.data, name) ?? findKey(result.data, 'id')) : undefined;
    const below = template(parts.slice(0, at));
    if (result.ok && value === undefined && this.lists.has(below)) {
      const items = await this.list(call, fillPath(below, values));
      value = itemValue(items.at(-1), name) ?? undefined;
    }
    if (!value) {
      throw new Unresolved(`${route.label} gave no ${name}: ${result.ok ? 'no id in the answer' : describe(result.error)}`);
    }
    return value;
  }

  /** Fill the path of a write: new records for ids, created children for the others. */
  async writeValues(call, op, way, values = {}) {
    const names = pathParameters(op.path);
    const parts = segments(op.path);
    const filled = { ...values };
    for (const name of names) {
      if (filled[name] !== undefined) {
        continue;
      }
      const at = parts.indexOf(`{${name}}`);
      if (at === 1) {
        const collection = `/${parts[0]}`;
        filled[name] = this.createRoute(collection)
          ? await this.newRecord(call, collection, way)
          : (await this.candidates(call, op, names, 0, {}))[0];
        if (!filled[name]) {
          throw new Unresolved(`the collection ${collection} has no record to act on`);
        }
      } else {
        filled[name] = await this.produce(call, op, name, filled, way);
      }
    }
    return filled;
  }

  /** One write request, with all setup writes done. */
  async writeRequest(call, op, values, way) {
    await this.loadSamples(call);
    if (op.method === 'post' && pathParameters(op.path).length === 0 && this.lists.has(op.path)) {
      return this.createRequest(call, op.path, way);
    }
    const filled = await this.writeValues(call, op, way, values);
    const path = fillPath(op.path, filled);
    const verb = segments(op.path).at(-1);
    if (BEFORE[verb]) {
      const before = this.find('post', op.path.replace(/[^/]+$/, BEFORE[verb]));
      // A record with nothing to undo needs no write before it.
      if (before && verb === 'submit' && !(await this.pending(call, op, filled))) {
        return this.requestFor(call, op, filled, path);
      }
      if (before) {
        const result = await call(await this.writeRequest(call, before, filled, way));
        if (!result.ok) {
          throw new Unresolved(`the ${BEFORE[verb]} before ${verb} failed: ${describe(result.error)}`);
        }
      }
    }
    return this.requestFor(call, op, filled, path);
  }

  /** The pending submission of the record that a write addresses, or undefined. */
  async pending(call, op, values) {
    const owner = this.memberPath(op, values);
    const record = owner ? await call({ method: 'get', path: owner }) : null;
    return record?.ok ? pendingValue(record.data, 'proposal_id') : undefined;
  }

  async requestFor(call, op, filled, path) {
    const headers = {};
    if (op.parameters.some(item => item.name === 'If-Match')) {
      headers['If-Match'] = await this.etag(call, this.memberPath(op, filled) ?? path);
    }
    const body = requestBody(op.requestSchema, await this.bodySeed(call, op, filled), this.bodyContext(null));
    if (op.method === 'patch' && body) {
      const field = patchField(op.requestSchema);
      if (field) {
        body[field] = this.counter++ % 2 ? this.context.text : this.context.text2;
      }
    }
    return { method: op.method, path, query: {}, headers, body };
  }

  async bodySeed(call, op, values) {
    const member = this.memberPath(op, values);
    if (!member) {
      return null;
    }
    const result = await call({ method: 'get', path: member });
    return result.ok ? result.data : null;
  }

  /** The concrete path of the record that a write changes, or null. */
  memberPath(op, values) {
    const parts = segments(op.path);
    for (let length = parts.length; length > 1; length--) {
      const candidate = template(parts.slice(0, length));
      if (candidate.endsWith('}') && this.find('get', candidate)) {
        return fillPath(candidate, values);
      }
    }
    return null;
  }

  /** The read that follows a write: the record it created or changed. */
  followRead(request, result) {
    const created = request.method === 'post' && this.lists.has(request.path) ? (request.body?.id ?? findKey(result.data, 'id')) : null;
    if (created) {
      return { method: 'get', path: `${request.path}/${encodeURIComponent(created)}`, query: {} };
    }
    const parts = segments(request.path);
    const routes = this.operations.filter(op => op.method === 'get' && !op.selector);
    for (let length = parts.length; length > 1; length--) {
      const concrete = parts.slice(0, length);
      const route = routes.find(op => {
        const shape = segments(op.path);
        return shape.length === length && shape.every((part, index) => part.startsWith('{') || part === concrete[index]) && shape.at(-1).startsWith('{');
      });
      if (route) {
        return { method: 'get', path: template(concrete), query: {} };
      }
    }
    return null;
  }
}

/** The value of `name` in the pending submission of a record, for example its proposal id. */
export function pendingValue(record, name) {
  const pending = findKey(record, 'pending');
  const nested = pending !== null && typeof pending === 'object' ? findKey(pending, name) : undefined;
  return nested ?? findKey(record, `pending_${name}`);
}

/** A short text for a failure answer. */
export function describe(error) {
  if (!error) {
    return 'no answer';
  }
  if (typeof error === 'string') {
    return error.slice(0, 300);
  }
  const parts = [error.kind, error.reason, error.field, error.message, error.code].filter(item => item !== undefined && item !== null);
  return (parts.length ? parts.join(' ') : JSON.stringify(error)).slice(0, 300);
}

export { resolveRef };
