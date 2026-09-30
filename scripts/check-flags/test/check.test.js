import assert from 'node:assert/strict';
import { readdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { checkFlags } from '../check.js';

const here = dirname(fileURLToPath(import.meta.url));
const fixtures = join(here, 'fixtures');
const schemaPath = join(here, '..', '..', '..', 'config', 'flags', 'schema', 'flags.json');

function run(fixture, tags = []) {
  return checkFlags({
    releasePath: join(fixtures, fixture, 'flags.flagd.json'),
    devPath: join(fixtures, fixture, 'dev.flagd.json'),
    schemaPath,
    tags,
  });
}

// Each failing fixture and a piece of the one error it must produce
const failures = {
  'invalid-json': 'release: invalid JSON',
  'schema-missing-variants': '$.flags.mcp: missing required property "variants"',
  'schema-bad-targeting': 'release: flagd schema: $.flags.world_map',
  'unknown-default-variant': 'flag "mcp": defaultVariant must name one of its variants',
  'bad-key': 'flag "worldMap": key must be snake_case',
  'missing-metadata': 'flag "mcp": metadata.kind must be one of',
  'missing-kind': 'flag "mcp": metadata.kind must be one of',
  'unknown-kind': 'flag "mcp": metadata.kind must be one of',
  'missing-owner': 'flag "mcp": metadata.owner must be the Linear issue',
  'owner-not-linear': 'flag "mcp": metadata.owner must be the Linear issue',
  'owner-roadmap-doc': 'flag "mcp": metadata.owner must be the Linear issue',
  'preview-missing-stage': 'flag "world_map": preview flags need metadata.stage',
  'preview-unknown-stage': 'flag "world_map": preview flags need metadata.stage',
  'stage-on-build-flag': 'flag "build_canary": metadata.stage is only for preview flags',
  'build-missing-remove-by': 'flag "build_canary": build flags need metadata.remove_by',
  'preview-missing-remove-by': 'flag "world_map": preview flags need metadata.remove_by',
  'malformed-remove-by': 'flag "build_canary": build flags need metadata.remove_by',
  'remove-by-on-ops-flag': 'flag "mcp": ops flags are long-lived',
  'build-non-boolean': 'flag "build_canary": build flags must have exactly the variants',
  'build-with-targeting': 'flag "build_canary": build flags are resolved at build time',
  'build-disabled': 'flag "build_canary": build flags must be ENABLED',
  'dev-unknown-flag': 'dev: flag "no_such_flag": overrides a flag that flags.flagd.json does not define',
  'dev-with-metadata': 'dev: flag "build_canary": metadata belongs in flags.flagd.json only',
  'dev-changes-variants': 'dev: flag "build_canary": variants must match flags.flagd.json',
  'dev-unknown-default-variant': 'dev: flag "build_canary": defaultVariant must name one of its variants',
  'dev-schema-invalid': '$.flags.build_canary.state: must be one of',
  'inherited-default-variant': 'flag "mcp": defaultVariant must name one of its variants',
  'unknown-metadata-key': 'flag "build_canary": unknown metadata key "removeBy"',
  'experiment-malformed-remove-by': 'flag "layout_test": bad metadata.remove_by',
  'dev-changes-state': 'dev: flag "build_canary": state must match flags.flagd.json',
  'dev-with-targeting':
    'dev: flag "build_canary": an override takes only state, variants, defaultVariant, not "targeting"',
};
const passing = ['valid', 'valid-dev-variants-reordered'];

test('every fixture directory has a test', () => {
  const dirs = readdirSync(fixtures).sort();
  assert.deepEqual(dirs, [...passing, ...Object.keys(failures)].sort());
});

for (const fixture of passing) {
  test(`${fixture} passes`, () => {
    assert.deepEqual(run(fixture), []);
  });
}

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

test('a flag whose remove_by train has shipped fails', () => {
  // valid has build_canary remove_by "train 1" and world_map "train 3"
  assert.deepEqual(run('valid', ['train-0', 'train-1-rc', 'train-10x']), []);
  assert.deepEqual(run('valid', ['train-1']), [
    'release: flag "build_canary": remove_by is train 1, and train-1 has shipped; delete the flag',
  ]);
});

test('a later train shipping counts when train N was never tagged', () => {
  assert.deepEqual(run('valid', ['train-4', 'train-2']), [
    'release: flag "build_canary": remove_by is train 1, and train-2 has shipped; delete the flag',
    'release: flag "world_map": remove_by is train 3, and train-4 has shipped; delete the flag',
  ]);
});

test('version tags from before release trains are ignored', () => {
  assert.deepEqual(run('valid', ['v0.2', 'v0.4.0', 'v1.0.0', 'v3.7.3', 'v2.6.0-professional-demo']), []);
});

test('the repository flag files pass', () => {
  const flagsDir = join(here, '..', '..', '..', 'config', 'flags');
  const errors = checkFlags({
    releasePath: join(flagsDir, 'flags.flagd.json'),
    devPath: join(flagsDir, 'dev.flagd.json'),
    schemaPath,
    tags: [],
  });
  assert.deepEqual(errors, []);
});
