#!/usr/bin/env node
// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

// Proves build-time flags compile code out. The `build_canary` flag is off in release and
// on in dev, and guards a lazily imported page (src/pages/FlagCanary.tsx) holding MARKER.
// The release bundle must not contain MARKER; the dev bundle must.

import { execFileSync } from 'node:child_process';
import { mkdtempSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const MARKER = 'econgraph-flag-canary-5b1e9d';

function filesUnder(dir) {
  return readdirSync(dir, { recursive: true, withFileTypes: true })
    .filter(entry => entry.isFile())
    .map(entry => join(entry.parentPath, entry.name));
}

function build(profile, outDir) {
  execFileSync(
    'npx',
    ['vite', 'build', '--outDir', outDir, '--emptyOutDir', '--logLevel', 'warn'],
    {
      stdio: 'inherit',
      env: { ...process.env, FLAGS_PROFILE: profile },
    }
  );
  return filesUnder(outDir).filter(file => readFileSync(file, 'utf8').includes(MARKER));
}

const root = mkdtempSync(join(tmpdir(), 'flag-bundle-'));
let failed = false;
try {
  const inRelease = build('release', join(root, 'release'));
  if (inRelease.length > 0) {
    console.error(
      `FAIL: release build contains the canary marker in:\n  ${inRelease.join('\n  ')}`
    );
    failed = true;
  } else {
    console.log('ok: release build has no canary code');
  }

  const inDev = build('dev', join(root, 'dev'));
  if (inDev.length === 0) {
    console.error('FAIL: dev build lacks the canary marker; the check is not testing anything');
    failed = true;
  } else {
    console.log('ok: dev build has the canary page');
  }
} finally {
  rmSync(root, { recursive: true, force: true });
}
process.exit(failed ? 1 : 0);
