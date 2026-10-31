import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  cpSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readlinkSync,
  rmSync,
  symlinkSync,
} from 'node:fs';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const frontend = resolve(dirname(fileURLToPath(import.meta.url)), '..');

test('the optimized E2E runtime retains the installed local matcher target', () => {
  const runtime = mkdtempSync(join(tmpdir(), 'econgraph-e2e-matcher-'));
  try {
    mkdirSync(join(runtime, 'node_modules'));
    const target = readlinkSync(join(frontend, 'node_modules/minimatch'));
    assert.equal(target, '../compat/minimatch');
    symlinkSync(target, join(runtime, 'node_modules/minimatch'));

    // Copy the actual installed registry dependency closure, without the large
    // frontend tree or any fallback to its node_modules during runtime import.
    const copied = new Set();
    function copyRegistryPackage(name) {
      if (copied.has(name)) return;
      copied.add(name);
      const source = join(frontend, 'node_modules', name);
      cpSync(source, join(runtime, 'node_modules', name), { recursive: true });
      const manifest = JSON.parse(readFileSync(join(source, 'package.json'), 'utf8'));
      for (const dependency of Object.keys(manifest.dependencies ?? {}))
        copyRegistryPackage(dependency);
    }
    copyRegistryPackage('minimatch-patched');
    const runtimeRequire = createRequire(join(runtime, 'consumer.cjs'));
    assert.throws(() => runtimeRequire('./node_modules/minimatch'), { code: 'MODULE_NOT_FOUND' });

    const dockerfile = readFileSync(
      join(frontend, '../ci/docker/e2e/Dockerfile.optimized'),
      'utf8'
    );
    const finalStage = dockerfile.split(/^FROM .*$/m).at(-1);
    assert.match(
      finalStage,
      /^COPY --from=frontend-builder \/app\/node_modules \.\/node_modules$/m
    );
    if (/^COPY --from=frontend-builder \/app\/compat \.\/compat$/m.test(finalStage))
      cpSync(join(frontend, 'compat'), join(runtime, 'compat'), { recursive: true });

    const match = runtimeRequire('./node_modules/minimatch');
    assert.equal(match('src/Chart.test.tsx', '**/*.test.{ts,tsx}'), true);
    assert.equal(match('src/Chart.tsx', '**/*.test.{ts,tsx}'), false);
  } finally {
    rmSync(runtime, { recursive: true, force: true });
  }
});
