// The rules for config/flags beyond flagd's own schema. See
// config/flags/README.md for what each flag must carry and why.

import { readFileSync } from 'node:fs';
import { SchemaValidator } from './schema.js';

const KINDS = ['build', 'preview', 'ops', 'experiment'];
const STAGES = ['alpha', 'beta'];
// Flag keys become __FLAG_WORLD_MAP__ in the frontend and cfg(flag_world_map) in the backend
const KEY = /^[a-z][a-z0-9]*(_[a-z0-9]+)*$/;
const OWNER = /^docs\/roadmap\/[a-z0-9-]+\.md$/;
const REMOVE_BY = /^(train ([1-9][0-9]*)|unscheduled)$/;

// Train N ships as v0.(N+1).0: train 1 is v0.2.0.
export function trainTag(train) {
  return `v0.${train + 1}.0`;
}

function readJson(path, label, errors) {
  let text;
  try {
    text = readFileSync(path, 'utf8');
  } catch (e) {
    errors.push(`${label}: cannot read ${path}: ${e.message}`);
    return undefined;
  }
  try {
    return JSON.parse(text);
  } catch (e) {
    errors.push(`${label}: invalid JSON: ${e.message}`);
    return undefined;
  }
}

function checkDefaultVariant(label, name, flag, errors) {
  if (typeof flag.defaultVariant !== 'string' || !(flag.defaultVariant in flag.variants)) {
    errors.push(`${label}: flag "${name}": defaultVariant must name one of its variants`);
  }
}

function checkReleaseFlag(name, flag, { docExists, tags }, errors) {
  const at = `release: flag "${name}"`;
  if (!KEY.test(name)) errors.push(`${at}: key must be snake_case: lowercase letters, digits and single underscores`);
  checkDefaultVariant('release', name, flag, errors);

  const meta = flag.metadata ?? {};
  const { kind, owner, stage, remove_by: removeBy } = meta;
  if (!KINDS.includes(kind)) errors.push(`${at}: metadata.kind must be one of ${KINDS.join(', ')}`);

  if (typeof owner !== 'string' || !OWNER.test(owner)) {
    errors.push(`${at}: metadata.owner must be the roadmap doc that owns the flag, like docs/roadmap/feature-flags.md`);
  } else if (!docExists(owner)) {
    errors.push(`${at}: metadata.owner ${owner} does not exist`);
  }

  if (kind === 'preview') {
    if (!STAGES.includes(stage)) errors.push(`${at}: preview flags need metadata.stage (${STAGES.join(' or ')})`);
  } else if (stage !== undefined) {
    errors.push(`${at}: metadata.stage is only for preview flags`);
  }

  if (kind === 'build' || kind === 'preview') {
    const m = typeof removeBy === 'string' ? REMOVE_BY.exec(removeBy) : null;
    if (!m) {
      errors.push(`${at}: ${kind} flags need metadata.remove_by ("train N" or "unscheduled")`);
    } else if (m[2]) {
      const tag = trainTag(Number(m[2]));
      if (tags.has(tag)) errors.push(`${at}: remove_by is ${removeBy}, which shipped as ${tag}; delete the flag`);
    }
  } else if (kind === 'ops' && removeBy !== undefined) {
    errors.push(`${at}: ops flags are long-lived and take no metadata.remove_by`);
  }

  if (kind === 'build') {
    // Folded into the bundle as a constant, so there is nothing to target
    if (!Object.values(flag.variants).every((v) => typeof v === 'boolean')) {
      errors.push(`${at}: build flags must have boolean variants`);
    }
    if (flag.targeting !== undefined)
      errors.push(`${at}: build flags are resolved at build time and take no targeting`);
    if (flag.state !== 'ENABLED') errors.push(`${at}: build flags must be ENABLED; turn one off with defaultVariant`);
  }
}

function checkDevFlag(name, flag, release, errors) {
  const at = `dev: flag "${name}"`;
  const base = release[name];
  if (!base) {
    errors.push(`${at}: overrides a flag that flags.flagd.json does not define`);
    return;
  }
  checkDefaultVariant('dev', name, flag, errors);
  if (flag.metadata !== undefined) errors.push(`${at}: metadata belongs in flags.flagd.json only`);
  if (JSON.stringify(flag.variants) !== JSON.stringify(base.variants)) {
    errors.push(`${at}: variants must match flags.flagd.json; an override only changes the variant served`);
  }
}

// releasePath, devPath: the flag files. schemaPath: the vendored flagd flags.json.
// tags: the repository's git tags. docExists(path): whether a repo path exists.
// Returns a list of error strings, empty when everything passes.
export function checkFlags({ releasePath, devPath, schemaPath, tags, docExists }) {
  const errors = [];
  const validator = new SchemaValidator(schemaPath);
  const files = { release: readJson(releasePath, 'release', errors), dev: readJson(devPath, 'dev', errors) };

  for (const [label, doc] of Object.entries(files)) {
    if (doc === undefined) continue;
    const schemaErrors = validator.validate(doc);
    errors.push(...schemaErrors.map((e) => `${label}: flagd schema: ${e}`));
    if (schemaErrors.length) files[label] = undefined;
  }
  if (!files.release) return errors;

  const release = files.release.flags;
  for (const [name, flag] of Object.entries(release)) {
    checkReleaseFlag(name, flag, { docExists, tags: new Set(tags) }, errors);
  }
  if (files.dev) {
    for (const [name, flag] of Object.entries(files.dev.flags)) checkDevFlag(name, flag, release, errors);
  }
  return errors;
}
