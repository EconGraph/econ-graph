// The adapter contains API wiring only. Its parser/matcher remains the auditable registry package.
import { createHash } from 'node:crypto';
import { readFileSync, readdirSync, lstatSync } from 'node:fs';
import { join } from 'node:path';

const IMPLEMENTATION_INTEGRITY =
  'sha512-t+Vn6DX4wPCxS23y03C0uhF7S1jpi1GfqPrnlEipCJaA2ykiNbMkVlDutZtmvLfRcUN6R8AYVlzTsQrT4RZoLQ==';
const SOURCE_HASHES = {
  'index.cjs': '3f468f89fbb23588b75012f527e8d2efef63c8c708e54e15c08bcf1049987694',
  'index.mjs': 'e416f6edaaffb77dce00c35d529aeba6d266d017fec4c4b69a8759dac5942552',
  'index.d.ts': '74ea05852ab1b06b47cf6387354fbe92fe20828cca90818f673ec53ae8068690',
};
const readJson = path => JSON.parse(readFileSync(path, 'utf8'));
const requireCondition = (condition, message) => {
  if (!condition) throw new Error(`Minimatch compatibility guard: ${message}`);
};

export function validateMinimatchCompatibility(frontend) {
  const directory = join(frontend, 'compat/minimatch');
  const manifest = readJson(join(directory, 'package.json'));
  const root = readJson(join(frontend, 'package.json'));
  const lock = readJson(join(frontend, 'package-lock.json'));
  requireCondition(
    readdirSync(directory).sort().join('|') === 'index.cjs|index.d.ts|index.mjs|package.json',
    'unexpected implementation/build files in the local adapter'
  );
  for (const [name, checksum] of Object.entries(SOURCE_HASHES)) {
    const path = join(directory, name);
    requireCondition(!lstatSync(path).isSymbolicLink(), 'adapter source cannot be a symlink');
    requireCondition(
      createHash('sha256').update(readFileSync(path)).digest('hex') === checksum,
      `${name} differs from the reviewed API-only adapter`
    );
  }
  requireCondition(
    Object.keys(manifest).sort().join('|') ===
      'dependencies|description|engines|exports|license|main|name|private|type|types|version' &&
      manifest.name === 'minimatch' &&
      manifest.version === '11.0.1' &&
      manifest.private === true &&
      manifest.main === 'index.cjs' &&
      manifest.types === 'index.d.ts' &&
      manifest.type === 'module' &&
      manifest.license === 'MIT' &&
      JSON.stringify(manifest.engines) === JSON.stringify({ node: '>=26.0.0 <27' }) &&
      JSON.stringify(manifest.exports) ===
        JSON.stringify({
          '.': { types: './index.d.ts', import: './index.mjs', require: './index.cjs' },
          './package.json': './package.json',
        }) &&
      JSON.stringify(manifest.dependencies) ===
        JSON.stringify({ 'minimatch-patched': 'npm:minimatch@11.0.1' }),
    'adapter must depend only on the exact patched registry implementation, without scripts'
  );
  requireCondition(
    root.devDependencies.minimatch === 'file:compat/minimatch' &&
      root.overrides.minimatch === '$minimatch' &&
      root.overrides['glob@>=9 <14.0.2'] === '14.0.2',
    'legacy API routing or modern glob patch selection changed'
  );
  requireCondition(
    lock.packages['node_modules/minimatch']?.link === true &&
      lock.packages['node_modules/minimatch'].resolved === 'compat/minimatch',
    'lock must route minimatch consumers through the inspected local adapter'
  );
  const implementation = lock.packages['node_modules/minimatch-patched'];
  requireCondition(
    implementation?.name === 'minimatch' &&
      implementation.version === '11.0.1' &&
      implementation.resolved === 'https://registry.npmjs.org/minimatch/-/minimatch-11.0.1.tgz' &&
      implementation.integrity === IMPLEMENTATION_INTEGRITY &&
      lock.packages['compat/minimatch']?.dependencies?.['minimatch-patched'] ===
        'npm:minimatch@11.0.1',
    'the actual patched registry package must stay in the integrity-checked, audited graph'
  );
  for (const [name, pkg] of Object.entries(lock.packages)) {
    if (name.endsWith('/minimatch') && !pkg.link) {
      requireCondition(
        pkg.version === '11.0.1',
        `old minimatch implementation returned at ${name}`
      );
    }
    if (name.endsWith('/glob') && Number(pkg.version?.split('.')[0]) >= 9) {
      requireCondition(pkg.version === '14.0.2', `unreviewed modern glob version at ${name}`);
    }
  }
  return {
    implementation: 'minimatch@11.0.1',
    registryAudited: true,
    legacyConsumerApiAdapted: true,
  };
}
