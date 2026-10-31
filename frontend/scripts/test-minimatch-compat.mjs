import assert from 'node:assert/strict';
import { test } from 'node:test';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync, cpSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';
import { execFileSync } from 'node:child_process';
import compatibility, { minimatch, Minimatch } from '../compat/minimatch/index.mjs';
import { glob } from 'glob';
import { validateMinimatchCompatibility } from './check-minimatch-compat.mjs';

const frontend = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const require = createRequire(import.meta.url);

test('legacy callable/default and modern named APIs use the patched matcher', () => {
  const commonjs = require('minimatch');
  assert.equal(typeof commonjs, 'function');
  assert.equal(commonjs('src/Chart.test.tsx', '**/*.test.{ts,tsx}'), true);
  assert.equal(compatibility('src/Chart.tsx', '**/*.test.{ts,tsx}'), false);
  assert.equal(minimatch('src/Chart.tsx', 'src/**/*.tsx'), true);
  assert.equal(new Minimatch('src/**/*.tsx').match('src/Chart.tsx'), true);
  assert.equal(commonjs.defaults({ nocase: true })('CHART.TSX', '*.tsx'), true);
  assert.equal(commonjs.makeRe('*.tsx').test('Chart.tsx'), true);
});

test('real glob7 callback/sync and patched modern glob keep file/ignore semantics', async () => {
  const root = mkdtempSync(join(tmpdir(), 'econgraph-glob-compat-'));
  try {
    mkdirSync(join(root, 'src'));
    for (const file of ['Chart.tsx', 'Chart.test.tsx', '.hidden.tsx'])
      writeFileSync(join(root, 'src', file), 'fixture');
    const legacyRequire = createRequire(join(frontend, 'node_modules/rimraf/package.json'));
    const legacy = legacyRequire('glob');
    assert.equal(legacyRequire('glob/package.json').version, '7.2.3');
    const options = { cwd: root, nodir: true, ignore: ['**/*.test.tsx'] };
    assert.deepEqual(legacy.sync('src/**/*.tsx', options), ['src/Chart.tsx']);
    const callback = await new Promise((accept, reject) =>
      legacy('src/**/*.tsx', options, (error, files) => (error ? reject(error) : accept(files)))
    );
    assert.deepEqual(callback, ['src/Chart.tsx']);
    assert.deepEqual(await glob('src/**/*.tsx', options), ['src/Chart.tsx']);
    assert.deepEqual((await glob('src/**/*.tsx', { ...options, dot: true })).sort(), [
      'src/.hidden.tsx',
      'src/Chart.tsx',
    ]);
    const controller = new AbortController();
    controller.abort(new Error('bounded-walk'));
    await assert.rejects(glob('**/*', { cwd: root, signal: controller.signal }), /bounded-walk/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('the real ESLint import rule consumes legacy makeRe without old regex implementation', () => {
  const { Linter } = require('eslint');
  const plugin = require('eslint-plugin-import');
  const diagnostics = new Linter().verify("import example from 'package/private/file';", [
    {
      plugins: { import: plugin },
      rules: { 'import/no-internal-modules': ['error', { forbid: ['**/private/**'] }] },
    },
  ]);
  assert.equal(diagnostics.length, 1);
  assert.equal(diagnostics[0].ruleId, 'import/no-internal-modules');
});

test('the source/lock guard accepts the real reviewed dependency graph', () => {
  assert.equal(validateMinimatchCompatibility(frontend).registryAudited, true);
});

for (const mutation of ['source', 'implementation', 'script']) {
  test(`the source/lock guard refuses ${mutation} substitution`, () => {
    const root = mkdtempSync(join(tmpdir(), 'econgraph-adapter-guard-'));
    try {
      cpSync(join(frontend, 'compat'), join(root, 'compat'), { recursive: true });
      for (const file of ['package.json', 'package-lock.json'])
        cpSync(join(frontend, file), join(root, file));
      const path = join(
        root,
        mutation === 'source'
          ? 'compat/minimatch/index.cjs'
          : mutation === 'script'
            ? 'compat/minimatch/package.json'
            : 'package-lock.json'
      );
      if (mutation === 'source')
        writeFileSync(path, readFileSync(path, 'utf8') + '\nmodule.exports = () => true;\n');
      else {
        const data = JSON.parse(readFileSync(path));
        if (mutation === 'script') data.scripts = { postinstall: 'unreviewed' };
        else data.packages['node_modules/minimatch-patched'].version = '10.2.6';
        writeFileSync(path, JSON.stringify(data));
      }
      assert.throws(() => validateMinimatchCompatibility(root), /Minimatch compatibility guard/);
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  });
}

test('real TypeScript consumers see the adapted named and default APIs', () => {
  // Keep the fixture below node_modules so package resolution uses the actual
  // installed minimatch exports and declaration, rather than a test-only path.
  const root = mkdtempSync(join(frontend, 'node_modules/.minimatch-types-'));
  try {
    const path = join(root, 'consumer.mts');
    writeFileSync(
      path,
      `
      import compatibility, { makeRe, minimatch, Minimatch, type MinimatchOptions } from 'minimatch';
      const options: MinimatchOptions = { dot: true };
      const results: boolean[] = [
        compatibility('Chart.tsx', '*.tsx', options),
        minimatch('Chart.tsx', '*.tsx', options),
        makeRe('*.tsx', options).test('Chart.tsx'),
        new Minimatch('*.tsx', options).makeRe().test('Chart.tsx'),
        new compatibility.Minimatch('*.tsx', options).makeRe().test('Chart.tsx'),
        new minimatch.Minimatch('*.tsx', options).makeRe().test('Chart.tsx'),
        compatibility.makeRe('*.tsx', options).test('Chart.tsx'),
        minimatch.makeRe('*.tsx', options).test('Chart.tsx'),
      ];
      const matched: string[] | null = makeRe('*.tsx')[Symbol.match]('Chart.tsx');
      // The adapter exposes only test/match, not the removed RegExp implementation.
      // @ts-expect-error Legacy regex source is intentionally unsupported.
      makeRe('*.tsx').source;
      export { results, matched };
    `
    );
    execFileSync(
      process.execPath,
      [
        require.resolve('typescript/lib/tsc.js'),
        '--ignoreConfig',
        '--noEmit',
        '--strict',
        '--skipLibCheck',
        '--module',
        'NodeNext',
        '--moduleResolution',
        'NodeNext',
        '--target',
        'ES2022',
        '--types',
        'node',
        path,
      ],
      { cwd: frontend, stdio: 'inherit' }
    );
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
