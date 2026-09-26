#!/usr/bin/env node
// Checks config/flags: flagd schema (vendored, no network), flag metadata,
// dev overrides, and flags whose remove_by train has shipped.
// Usage: node scripts/check-flags [--tags train-1,train-2]
// Without --tags it reads the repository's tags from git, so CI needs them
// fetched (actions/checkout with fetch-depth: 0).

import { execFileSync } from 'node:child_process';
import { existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { checkFlags } from './check.js';

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const flagsDir = join(root, 'config', 'flags');

function fail(message) {
  console.error(`check-flags: ${message}`);
  process.exit(2);
}

function git(...args) {
  try {
    return execFileSync('git', args, { cwd: root, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] }).trim();
  } catch (e) {
    fail(`git ${args.join(' ')} failed; pass --tags explicitly outside a git checkout (${e.message.split('\n')[0]})`);
  }
}

function gitTags() {
  const tags = git('tag', '--list', 'train-*').split('\n').filter(Boolean);
  // In a shallow clone the tags may be missing, so the remove_by check can't fire
  if (git('rev-parse', '--is-shallow-repository') === 'true') {
    console.warn('check-flags: warning: shallow clone; the remove_by check may miss a shipped train');
  }
  return tags;
}

const args = process.argv.slice(2);
if (!(args.length === 0 || (args.length === 2 && args[0] === '--tags' && !args[1].startsWith('--')))) {
  fail('usage: node scripts/check-flags [--tags train-1,train-2]');
}
const tags = args.length ? args[1].split(',').filter(Boolean) : gitTags();

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
