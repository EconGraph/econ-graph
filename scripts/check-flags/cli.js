#!/usr/bin/env node
// Checks config/flags: flagd schema (vendored, no network), flag metadata,
// dev overrides, and flags whose remove_by train has shipped.
// Usage: node scripts/check-flags [--tags train-1,train-2] [--shipped-train N]
// Without --tags it reads the repository's tags from git, so CI needs them
// fetched (actions/checkout with fetch-depth: 0).

import { execFileSync } from 'node:child_process';
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

// --shipped-train N checks as if train N's tag already existed, so a release can
// find the flags it must delete before it pushes train-N
const usage = 'usage: node scripts/check-flags [--tags train-1,train-2] [--shipped-train N]';
const opts = {};
const args = process.argv.slice(2);
for (let i = 0; i < args.length; i += 2) {
  const [flag, value] = [args[i], args[i + 1]];
  if (!['--tags', '--shipped-train'].includes(flag) || value === undefined || value.startsWith('--') || flag in opts) {
    fail(usage);
  }
  opts[flag] = value;
}
if ('--shipped-train' in opts && !/^[1-9][0-9]*$/.test(opts['--shipped-train'])) fail(usage);
const tags = '--tags' in opts ? opts['--tags'].split(',').filter(Boolean) : gitTags();
if ('--shipped-train' in opts) tags.push(`train-${opts['--shipped-train']}`);

const errors = checkFlags({
  releasePath: join(flagsDir, 'flags.flagd.json'),
  devPath: join(flagsDir, 'dev.flagd.json'),
  schemaPath: join(flagsDir, 'schema', 'flags.json'),
  tags,
});

if (errors.length) {
  console.error(`check-flags: ${errors.length} problem(s) in config/flags:`);
  for (const e of errors) console.error(`  ${e}`);
  process.exit(1);
}
console.log('check-flags: config/flags is valid');
