// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

// Build-time feature flags (docs/build-flags.md).
//
// Reads flags/<profile>.json, which scripts/flags-merge generates from config/flags, and
// turns every `kind: build` flag into a `define` entry `__FLAGS__.<name>` = true|false.
// The bundler folds the constant, so code behind a flag that is off is dropped from the
// bundle. This is the only runtime reader of the flag files; application code reads
// `__FLAGS__.<name>` (typed in src/flags.ts).

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import type { Plugin } from 'vite';

export type FlagProfile = 'release' | 'dev';

const PROFILES: readonly FlagProfile[] = ['release', 'dev'];

interface ResolvedFlags {
  profile?: string;
  flags?: Record<string, { kind?: string; value?: unknown }>;
}

/** Directory holding the generated per-profile flag files. */
export const FLAGS_DIR = resolve(import.meta.dirname, 'flags');

/**
 * Picks the flag profile. `FLAGS_PROFILE` wins; otherwise `vite build` is `release` and
 * everything else (dev server, preview, vitest) is `dev`.
 */
export function resolveFlagProfile(command: 'build' | 'serve'): FlagProfile {
  const fromEnv = process.env.FLAGS_PROFILE;
  if (fromEnv) {
    if (!PROFILES.includes(fromEnv as FlagProfile)) {
      throw new Error(`FLAGS_PROFILE must be one of ${PROFILES.join(', ')}, got "${fromEnv}"`);
    }
    return fromEnv as FlagProfile;
  }
  return command === 'build' ? 'release' : 'dev';
}

/** Reads the `kind: build` flags of a profile as name -> boolean. */
export function loadBuildFlags(
  profile: FlagProfile,
  dir: string = FLAGS_DIR
): Record<string, boolean> {
  const path = resolve(dir, `${profile}.json`);
  let file: ResolvedFlags;
  try {
    file = JSON.parse(readFileSync(path, 'utf8')) as ResolvedFlags;
  } catch (err) {
    throw new Error(
      `Cannot read flag file ${path} (run node scripts/flags-merge): ${(err as Error).message}`
    );
  }
  if (file.profile !== profile) {
    throw new Error(`Flag file ${path} is for profile "${file.profile}", expected "${profile}"`);
  }

  const result: Record<string, boolean> = {};
  for (const [name, flag] of Object.entries(file.flags ?? {})) {
    if (flag.kind !== 'build') continue;
    if (typeof flag.value !== 'boolean') {
      throw new Error(`Build flag "${name}" in ${path} is not a boolean`);
    }
    result[name] = flag.value;
  }
  return result;
}

/**
 * Fails a build whose output still reads `__FLAGS__`. That happens when code reads a flag
 * that has no `define` (a typo, or a flag that isn't `kind: build`): the read would throw
 * a ReferenceError when the page loads, since `__FLAGS__` exists only in the dev server.
 */
export function flagsGuard(): Plugin {
  return {
    name: 'econgraph-flags-guard',
    apply: 'build',
    generateBundle(_options, bundle) {
      const leaks = Object.values(bundle)
        .filter(output => output.type === 'chunk' && output.code.includes('__FLAGS__'))
        .map(output => output.fileName);
      if (leaks.length > 0) {
        this.error(
          `Unreplaced __FLAGS__ in ${leaks.join(', ')}: a flag read has no build-time value. ` +
            'Only kind: build flags in flags/<profile>.json are defined.'
        );
      }
    },
  };
}

/** `define` entries for a profile: `__FLAGS__.<name>` for each build flag. */
export function flagDefines(profile: FlagProfile, dir?: string): Record<string, string> {
  return Object.fromEntries(
    Object.entries(loadBuildFlags(profile, dir)).map(([name, on]) => [
      `__FLAGS__.${name}`,
      JSON.stringify(on),
    ])
  );
}
