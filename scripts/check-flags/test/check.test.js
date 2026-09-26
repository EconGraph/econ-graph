import assert from 'node:assert/strict';
import { readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { checkFlags, trainTag } from '../check.js';

const here = dirname(fileURLToPath(import.meta.url));
const fixtures = join(here, 'fixtures');
const schemaPath = join(here, '..', '..', '..', 'config', 'flags', 'schema', 'flags.json');
const docs = new Set(['docs/roadmap/feature-flags.md']);

function run(fixture, tags = []) {
  return checkFlags({
    releasePath: join(fixtures, fixture, 'flags.flagd.json'),
    devPath: join(fixtures, fixture, 'dev.flagd.json'),
    schemaPath,
    tags,
    docExists: (path) => docs.has(path),
  });
}

// Each failing fixture and a piece of the one error it must produce
const failures = {
  'invalid-json': 'release: invalid JSON',
  'schema-missing-variants': '$.flags.mcp: missing required property "variants"',
  'schema-bad-targeting': 'release: flagd schema: $.flags.worldMap',
  'unknown-default-variant': 'flag "mcp": defaultVariant must name one of its variants',
  'bad-key': 'flag "world-map": key must be camelCase',
  'missing-metadata': 'flag "mcp": metadata.kind must be one of',
  'missing-kind': 'flag "mcp": metadata.kind must be one of',
  'unknown-kind': 'flag "mcp": metadata.kind must be one of',
  'missing-owner': 'flag "mcp": metadata.owner must be the roadmap doc',
  'owner-not-a-roadmap-doc': 'flag "mcp": metadata.owner must be the roadmap doc',
  'owner-doc-missing': 'flag "mcp": metadata.owner docs/roadmap/no-such-doc.md does not exist',
  'preview-missing-stage': 'flag "worldMap": preview flags need metadata.stage',
  'preview-unknown-stage': 'flag "worldMap": preview flags need metadata.stage',
  'stage-on-build-flag': 'flag "buildCanary": metadata.stage is only for preview flags',
  'build-missing-remove-by': 'flag "buildCanary": build flags need metadata.remove_by',
  'preview-missing-remove-by': 'flag "worldMap": preview flags need metadata.remove_by',
  'malformed-remove-by': 'flag "buildCanary": build flags need metadata.remove_by',
  'remove-by-on-ops-flag': 'flag "mcp": ops flags are long-lived',
  'build-non-boolean': 'flag "buildCanary": build flags must have boolean variants',
  'build-with-targeting': 'flag "buildCanary": build flags are resolved at build time',
  'build-disabled': 'flag "buildCanary": build flags must be ENABLED',
  'dev-unknown-flag': 'dev: flag "noSuchFlag": overrides a flag that flags.flagd.json does not define',
  'dev-with-metadata': 'dev: flag "buildCanary": metadata belongs in flags.flagd.json only',
  'dev-changes-variants': 'dev: flag "buildCanary": variants must match flags.flagd.json',
  'dev-unknown-default-variant': 'dev: flag "buildCanary": defaultVariant must name one of its variants',
  'dev-schema-invalid': '$.flags.buildCanary.state: must be one of',
};

test('every fixture directory has a test', () => {
  const dirs = readdirSync(fixtures).sort();
  assert.deepEqual(dirs, ['valid', ...Object.keys(failures)].sort());
});

test('valid fixture passes', () => {
  assert.deepEqual(run('valid'), []);
});

for (const [fixture, expected] of Object.entries(failures)) {
  test(`${fixture} fails`, () => {
    const errors = run(fixture);
    assert.ok(errors.length > 0, 'expected an error');
    assert.ok(
      errors.some((e) => e.includes(expected)),
      `expected an error containing:\n  ${expected}\ngot:\n  ${errors.join('\n  ')}`,
    );
  });
}

test('train N ships as v0.(N+1).0', () => {
  assert.equal(trainTag(1), 'v0.2.0');
  assert.equal(trainTag(3), 'v0.4.0');
});

test('a flag whose remove_by train has shipped fails', () => {
  // valid has buildCanary remove_by "train 1" and worldMap "train 3"
  assert.deepEqual(run('valid', ['v0.1.0', 'v0.2.0-rc.1']), []);
  const errors = run('valid', ['v0.1.0', 'v0.2.0']);
  assert.deepEqual(errors, [
    'release: flag "buildCanary": remove_by is train 1, which shipped as v0.2.0; delete the flag',
  ]);
});

test('the repository flag files pass', () => {
  const flagsDir = join(here, '..', '..', '..', 'config', 'flags');
  const errors = checkFlags({
    releasePath: join(flagsDir, 'flags.flagd.json'),
    devPath: join(flagsDir, 'dev.flagd.json'),
    schemaPath,
    tags: [],
    docExists: () => true,
  });
  assert.deepEqual(errors, []);
});
