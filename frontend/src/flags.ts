// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

/**
 * Build-time feature flags. See docs/build-flags.md.
 *
 * Read a flag as `__FLAGS__.<name>` directly, in the condition that guards the code:
 *
 *   const Page = __FLAGS__.world_map ? lazy(() => import('./pages/WorldMap')) : null;
 *   {Page && (
 *     <Route path='/world' element={<Suspense fallback={null}><Page /></Suspense>} />
 *   )}
 *
 * Vite replaces each `__FLAGS__.<name>` with `true` or `false` (flags.config.ts), so the
 * bundler drops the branch and the lazily imported module when the flag is off. Copying a
 * flag into a variable, object or prop defeats that: `__FLAGS__` itself exists only in the
 * dev server, and a copied value may not be folded.
 *
 * The names come from the generated flags/release.json (a type-only import, so nothing is
 * read at runtime), which makes an unknown flag name a type error. Only `kind: build`
 * flags get a value; the build fails if code reads any other (flagsGuard in
 * flags.config.ts).
 */
import type releaseFlags from '../flags/release.json';

/** A flag key in the generated flag file (every flag in train 1 is `kind: build`). */
export type FlagName = keyof (typeof releaseFlags)['flags'];

declare global {
  /** Build-time flags, one boolean constant per flag. */
  const __FLAGS__: Readonly<Record<FlagName, boolean>>;
}
