#!/usr/bin/env node
// Writes the resolved flag values for each profile to frontend/flags/<profile>.json
// and backend/flags/<profile>.json. Run after editing config/flags.
// Usage: node scripts/flags-merge [--check]
// --check writes nothing and fails if a generated file is missing or stale.
// Run node scripts/check-flags first: this assumes the flag files are valid.

import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { PROFILES, TARGETS, render, resolve } from './merge.js';

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const read = (name) => JSON.parse(readFileSync(join(root, 'config', 'flags', name), 'utf8'));
const release = read('flags.flagd.json');
const dev = read('dev.flagd.json');
const check = process.argv.includes('--check');

const stale = [];
for (const profile of PROFILES) {
  const content = render(resolve(release, dev, profile));
  for (const target of TARGETS) {
    const path = join(root, target, `${profile}.json`);
    const rel = `${target}/${profile}.json`;
    if (check) {
      let current;
      try {
        current = readFileSync(path, 'utf8');
      } catch {
        current = undefined;
      }
      if (current !== content) stale.push(rel);
    } else {
      mkdirSync(dirname(path), { recursive: true });
      writeFileSync(path, content);
      console.log(`flags-merge: wrote ${rel}`);
    }
  }
}

if (stale.length) {
  console.error(`flags-merge: generated flag files are stale: ${stale.join(', ')}`);
  console.error('Run node scripts/flags-merge and commit the result.');
  process.exit(1);
}
if (check) console.log('flags-merge: generated flag files are up to date');
