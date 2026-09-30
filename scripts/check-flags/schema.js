// A small JSON Schema (draft-07) validator: just the keywords the vendored
// flagd schemas in config/flags/schema use, so the flag check needs no npm
// dependencies and no network. An unsupported keyword is an error rather
// than silently ignored, so a schema update that needs more fails loudly.

import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';

const ANNOTATIONS = new Set([
  '$schema',
  '$id',
  '$comment',
  'title',
  'description',
  'definitions',
  'default',
  'examples',
  'readOnly',
  'writeOnly',
]);

function typeOf(value) {
  if (value === null) return 'null';
  if (Array.isArray(value)) return 'array';
  if (typeof value === 'number') return Number.isInteger(value) ? 'integer' : 'number';
  return typeof value;
}

function matchesType(value, type) {
  const actual = typeOf(value);
  return actual === type || (type === 'number' && actual === 'integer');
}

// JSON equality that ignores object key order
export function deepEqual(a, b) {
  if (typeOf(a) !== typeOf(b)) return false;
  if (Array.isArray(a)) return a.length === b.length && a.every((v, i) => deepEqual(v, b[i]));
  if (typeOf(a) === 'object') {
    const keys = Object.keys(a);
    return keys.length === Object.keys(b).length && keys.every((k) => Object.hasOwn(b, k) && deepEqual(a[k], b[k]));
  }
  return a === b;
}

function pointer(root, fragment) {
  let node = root;
  for (const raw of fragment.replace(/^\//, '').split('/').filter(Boolean)) {
    const part = decodeURIComponent(raw).replace(/~1/g, '/').replace(/~0/g, '~');
    if (node === null || typeof node !== 'object' || !Object.hasOwn(node, part)) return undefined;
    node = node[part];
  }
  return node;
}

export class SchemaValidator {
  // entryPath: the schema file to validate documents against. Relative $refs
  // ("./targeting.json") resolve against its directory, never the network.
  constructor(entryPath) {
    this.docs = new Map();
    this.entry = resolve(entryPath);
    this.load(this.entry);
  }

  load(file) {
    if (!this.docs.has(file)) this.docs.set(file, JSON.parse(readFileSync(file, 'utf8')));
    return this.docs.get(file);
  }

  resolveRef(ref, baseFile) {
    const [path, fragment = ''] = ref.split('#');
    if (/^[a-z]+:/i.test(path)) throw new Error(`schema $ref to a remote URI is not supported: ${ref}`);
    const file = path ? resolve(dirname(baseFile), path) : baseFile;
    const schema = pointer(this.load(file), fragment);
    if (schema === undefined) throw new Error(`unresolvable schema $ref: ${ref}`);
    return { schema, file };
  }

  // Returns a list of error strings, empty when the value is valid.
  validate(value) {
    return this.check(value, this.load(this.entry), this.entry, '$');
  }

  // Keywords beside a $ref are applied too (as in draft 2019-09 and later),
  // not ignored as draft-07 says. flags.json puts `properties` beside a $ref
  // for flag set metadata, and applying it is the stricter reading.
  check(value, schema, file, at) {
    if (schema === true || (typeof schema === 'object' && Object.keys(schema).length === 0)) return [];
    if (schema === false) return [`${at}: no value is allowed here`];
    const errors = [];
    for (const [keyword, arg] of Object.entries(schema)) {
      if (ANNOTATIONS.has(keyword)) continue;
      const fn = KEYWORDS[keyword];
      if (!fn) throw new Error(`unsupported JSON Schema keyword "${keyword}" in ${file}`);
      errors.push(...fn.call(this, value, arg, schema, file, at));
    }
    return errors;
  }
}

const KEYWORDS = {
  $ref(value, ref, _schema, file, at) {
    const target = this.resolveRef(ref, file);
    return this.check(value, target.schema, target.file, at);
  },
  type(value, type, _schema, _file, at) {
    const types = Array.isArray(type) ? type : [type];
    return types.some((t) => matchesType(value, t))
      ? []
      : [`${at}: expected ${types.join(' or ')}, got ${typeOf(value)}`];
  },
  enum(value, options, _schema, _file, at) {
    return options.some((o) => deepEqual(o, value)) ? [] : [`${at}: must be one of ${JSON.stringify(options)}`];
  },
  minLength(value, min, _schema, _file, at) {
    return typeof value === 'string' && [...value].length < min ? [`${at}: shorter than ${min}`] : [];
  },
  pattern(value, source, _schema, _file, at) {
    return typeof value === 'string' && !new RegExp(source, 'u').test(value)
      ? [`${at}: does not match /${source}/`]
      : [];
  },
  minimum(value, min, _schema, _file, at) {
    return typeof value === 'number' && value < min ? [`${at}: less than ${min}`] : [];
  },
  minItems(value, min, _schema, _file, at) {
    return Array.isArray(value) && value.length < min ? [`${at}: fewer than ${min} items`] : [];
  },
  maxItems(value, max, _schema, _file, at) {
    return Array.isArray(value) && value.length > max ? [`${at}: more than ${max} items`] : [];
  },
  items(value, items, _schema, file, at) {
    if (!Array.isArray(value)) return [];
    if (Array.isArray(items)) {
      return items.flatMap((s, i) => (i < value.length ? this.check(value[i], s, file, `${at}[${i}]`) : []));
    }
    return value.flatMap((v, i) => this.check(v, items, file, `${at}[${i}]`));
  },
  additionalItems(value, extra, schema, file, at) {
    if (!Array.isArray(value) || !Array.isArray(schema.items)) return [];
    return value
      .slice(schema.items.length)
      .flatMap((v, i) => this.check(v, extra, file, `${at}[${i + schema.items.length}]`));
  },
  minProperties(value, min, _schema, _file, at) {
    return typeOf(value) === 'object' && Object.keys(value).length < min ? [`${at}: fewer than ${min} properties`] : [];
  },
  required(value, names, _schema, _file, at) {
    if (typeOf(value) !== 'object') return [];
    return names.filter((n) => !Object.hasOwn(value, n)).map((n) => `${at}: missing required property "${n}"`);
  },
  properties(value, props, _schema, file, at) {
    if (typeOf(value) !== 'object') return [];
    return Object.entries(props).flatMap(([k, s]) =>
      Object.hasOwn(value, k) ? this.check(value[k], s, file, `${at}.${k}`) : [],
    );
  },
  patternProperties(value, patterns, _schema, file, at) {
    if (typeOf(value) !== 'object') return [];
    return Object.entries(patterns).flatMap(([p, s]) => {
      const re = new RegExp(p, 'u');
      return Object.keys(value)
        .filter((k) => re.test(k))
        .flatMap((k) => this.check(value[k], s, file, `${at}.${k}`));
    });
  },
  additionalProperties(value, extra, schema, file, at) {
    if (typeOf(value) !== 'object') return [];
    const named = new Set(Object.keys(schema.properties ?? {}));
    const patterns = Object.keys(schema.patternProperties ?? {}).map((p) => new RegExp(p, 'u'));
    return Object.keys(value)
      .filter((k) => !named.has(k) && !patterns.some((re) => re.test(k)))
      .flatMap((k) =>
        extra === false ? [`${at}: unexpected property "${k}"`] : this.check(value[k], extra, file, `${at}.${k}`),
      );
  },
  allOf(value, schemas, _schema, file, at) {
    return schemas.flatMap((s) => this.check(value, s, file, at));
  },
  anyOf(value, schemas, _schema, file, at) {
    const results = schemas.map((s) => this.check(value, s, file, at));
    if (results.some((r) => r.length === 0)) return [];
    return [`${at}: matches none of the allowed shapes`, ...closest(results)];
  },
  oneOf(value, schemas, _schema, file, at) {
    const results = schemas.map((s) => this.check(value, s, file, at));
    const passing = results.filter((r) => r.length === 0).length;
    if (passing === 1) return [];
    if (passing > 1) return [`${at}: matches more than one of the allowed shapes`];
    return [`${at}: matches none of the allowed shapes`, ...closest(results)];
  },
  not(value, schema, _schema, file, at) {
    return this.check(value, schema, file, at).length === 0 ? [`${at}: matches a disallowed shape`] : [];
  },
};

// Of several failed alternatives, report the one that got furthest (fewest
// errors), indented, so a typo in a flag doesn't bury the real problem.
function closest(results) {
  const best = results.reduce((a, b) => (b.length < a.length ? b : a));
  return best.map((e) => `  ${e}`);
}
