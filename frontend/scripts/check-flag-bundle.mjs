#!/usr/bin/env node
// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

// Proves build-time flags compile code out. Each canary below is a unique marker string
// rendered only by code gated on one flag; a build's output must contain the marker iff the
// flag's generated value (frontend/flags/<profile>.json) is true for that profile.
//
// `build_canary` is on in dev, so `requireOnSomewhere` also fails the check if the marker
// never shows up anywhere: a check that can never fail isn't proving anything. The
// `global_analysis_tabs` canaries (ECO-105) have no dev override (off in every profile, per
// FLAGS-2's rule for that flag), so they can't be "on" in a build here; `requireOnSomewhere`
// is false for them, and a regression (a static import defeating the flag gate) is still
// caught because the marker would then appear in both builds despite `value: false`.

import { execFileSync } from 'node:child_process';
import { mkdtempSync, readdirSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = dirname(dirname(fileURLToPath(import.meta.url)));

const CANARIES = [
  { flag: 'build_canary', marker: 'econgraph-flag-canary-5b1e9d', requireOnSomewhere: true },
  {
    flag: 'global_analysis_tabs',
    marker: 'econgraph-flag-global-analysis-tabs-compare-7c2f4a',
    requireOnSomewhere: false,
  },
  {
    flag: 'global_analysis_tabs',
    marker: 'econgraph-flag-global-analysis-tabs-events-7c2f4a',
    requireOnSomewhere: false,
  },
];

function filesUnder(dir) {
  return readdirSync(dir, { recursive: true, withFileTypes: true })
    .filter(entry => entry.isFile())
    .map(entry => join(entry.parentPath, entry.name));
}

function flagValue(profile, flag) {
  const generated = JSON.parse(readFileSync(join(root, 'flags', `${profile}.json`), 'utf8'));
  const entry = generated.flags[flag];
  if (!entry) {
    throw new Error(`check-flag-bundle: flag "${flag}" is not in flags/${profile}.json`);
  }
  return entry.value;
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
  const contents = filesUnder(outDir).map(file => readFileSync(file, 'utf8'));
  return marker => contents.some(text => text.includes(marker));
}

const tmpRoot = mkdtempSync(join(tmpdir(), 'flag-bundle-'));
let failed = false;
const seenOn = new Set();
try {
  for (const profile of ['release', 'dev']) {
    const hasMarker = build(profile, join(tmpRoot, profile));
    for (const canary of CANARIES) {
      const expected = flagValue(profile, canary.flag);
      const found = hasMarker(canary.marker);
      if (expected) seenOn.add(canary.marker);
      if (expected && !found) {
        console.error(
          `FAIL: ${profile} build is missing "${canary.marker}" though ${canary.flag} is on`
        );
        failed = true;
      } else if (!expected && found) {
        console.error(
          `FAIL: ${profile} build contains "${canary.marker}" though ${canary.flag} is off`
        );
        failed = true;
      } else {
        console.log(`ok: ${profile} build ${expected ? 'has' : 'lacks'} "${canary.marker}"`);
      }
    }
  }

  for (const canary of CANARIES) {
    if (canary.requireOnSomewhere && !seenOn.has(canary.marker)) {
      console.error(
        `FAIL: "${canary.marker}" is never expected on in any build; the check is not testing anything`
      );
      failed = true;
    }
  }
} finally {
  rmSync(tmpRoot, { recursive: true, force: true });
}
process.exit(failed ? 1 : 0);
