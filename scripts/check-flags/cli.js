#!/usr/bin/env node
// Checks config/flags: flagd schema (vendored, no network), flag metadata,
// dev overrides, and flags whose remove_by train has shipped.
// Usage: node scripts/check-flags [--tags v0.2.0,v0.3.0]
// Without --tags it reads the repository's tags from git, so CI needs them
// fetched (actions/checkout with fetch-depth: 0).

import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { checkFlags } from './check.js';

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const flagsDir = join(root, 'config', 'flags');

function gitTags() {
  const out = execFileSync('git', ['tag', '--list', 'v*'], { cwd: root, encoding: 'utf8' });
  return out.split('\n').filter(Boolean);
}

const args = process.argv.slice(2);
const tagsAt = args.indexOf('--tags');
const tags = tagsAt >= 0 ? (args[tagsAt + 1] ?? '').split(',').filter(Boolean) : gitTags();

const errors = checkFlags({
  releasePath: join(flagsDir, 'flags.flagd.json'),
  devPath: join(flagsDir, 'dev.flagd.json'),
  schemaPath: join(flagsDir, 'schema', 'flags.json'),
  tags,
  docExists: (path) => existsSync(join(root, path)),
});

if (errors.length) {
  console.error(`check-flags: ${errors.length} problem(s) in config/flags:`);
  for (const e of errors) console.error(`  ${e}`);
  process.exit(1);
}
console.log('check-flags: config/flags is valid');
