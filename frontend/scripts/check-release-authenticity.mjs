#!/usr/bin/env node
// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

// REL-3 "nothing fake" check (exit criterion 4): the release bundle must not ship
// placeholder/demo content as if it were real. Two complementary checks, because a build-time
// flag that fails to gate code the way `build-flags.md` describes still ends up in the bundle:
//
//  - Module reachability: every module Vite actually includes in the release build (not just
//    grepped text) is checked against MODULE_PATTERNS. This is what catches code a flag forgot
//    to gate: a minifier renames the `sampleCountryData` identifier away, so grepping the
//    output text for it finds nothing even though the module (and its fake country data) is
//    right there in the bundle. The build's own module graph can't be fooled by minification.
//  - Text markers: literal strings like "Coming Soon" are never renamed by a minifier, so a
//    plain substring search over the emitted code catches them even in a module the reachability
//    check doesn't otherwise flag.
//
// Known offenders are allowed via nothing-fake-allowlist.json, each entry with a reason and
// which PR removes it. REL-5 requires the allowlist to be empty before the version tag.

import { readFileSync, realpathSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = dirname(here);
const allowlistPath = join(here, 'nothing-fake-allowlist.json');

/** Module-path substrings that must not reach the release bundle. */
export const MODULE_PATTERNS = [
  {
    re: /\/src\/data\/samples?\/|\/src\/data\/sample[A-Za-z0-9]*\.(tsx?|jsx?|json)(\?|$)/,
    label: 'sample/demo data module',
  },
  {
    re: /\/src\/test-utils\/mocks\//,
    label: 'mock fixture module',
  },
];

/** Literal strings that must not appear in the release bundle's emitted code. */
export const TEXT_MARKERS = ['generateMock', 'SAMPLE_', 'sampleCountryData', 'Coming Soon'];

/**
 * Scans the module ids Vite included in a release build's chunks against MODULE_PATTERNS.
 * `moduleIds` is the flat list of absolute source paths across every emitted chunk.
 */
export function findModuleFindings(moduleIds, patterns = MODULE_PATTERNS) {
  const findings = [];
  for (const id of moduleIds) {
    for (const pattern of patterns) {
      if (pattern.re.test(id)) {
        findings.push({ type: 'module', key: toRelative(id), label: pattern.label });
      }
    }
  }
  return findings;
}

/**
 * Scans emitted chunk code (and any other text asset) for TEXT_MARKERS. `files` is an array of
 * `{ fileName, code }` for every chunk/text asset in the build output.
 */
export function findTextFindings(files, markers = TEXT_MARKERS) {
  const findings = [];
  for (const marker of markers) {
    const inFiles = files.filter(f => f.code.includes(marker)).map(f => f.fileName);
    if (inFiles.length > 0) {
      findings.push({ type: 'text', key: marker, label: `literal "${marker}"`, files: inFiles });
    }
  }
  return findings;
}

function toRelative(absPath) {
  const rel = relative(root, absPath);
  return rel.split('\\').join('/');
}

/** Loads the allowlist and rejects a malformed entry: an empty pattern would excuse every
 * finding of its type, and an entry with no reason/removeBy defeats the point of the list. */
export function loadAllowlist(path = allowlistPath) {
  const allowlist = JSON.parse(readFileSync(path, 'utf8'));
  for (const entry of allowlist) {
    for (const field of ['type', 'pattern', 'reason', 'removeBy']) {
      if (typeof entry[field] !== 'string' || entry[field].trim() === '') {
        throw new Error(
          `${path}: allowlist entry ${JSON.stringify(entry)} is missing a non-empty "${field}"`
        );
      }
    }
  }
  return allowlist;
}

/**
 * An allowlist entry excuses a finding of the same `type` whose `key` exactly matches the
 * entry's `pattern` (exact, not substring, so one entry can't quietly excuse a whole family of
 * future modules or every file a text marker turns up in).
 */
export function unallowedFindings(findings, allowlist) {
  return findings.filter(
    finding =>
      !allowlist.some(entry => entry.type === finding.type && entry.pattern === finding.key)
  );
}

/** Allowlist entries that excused nothing in this build — stale, and due for removal (the
 * area that fixed the underlying code should have deleted the entry in the same PR). */
export function staleAllowlistEntries(findings, allowlist) {
  return allowlist.filter(
    entry => !findings.some(finding => finding.type === entry.type && finding.key === entry.pattern)
  );
}

function describe(finding) {
  const extra = finding.files ? ` (in ${finding.files.join(', ')})` : '';
  return `${finding.type} finding: ${finding.label} — ${finding.key}${extra}`;
}

async function buildReleaseModuleGraph() {
  const { build } = await import('vite');
  const result = await build({
    configFile: join(root, 'vite.config.ts'),
    mode: 'production',
    logLevel: 'warn',
    build: { write: false, outDir: join(root, '.nothing-fake-tmp') },
  });
  const outputs = Array.isArray(result) ? result : [result];
  const moduleIds = [];
  const files = [];
  for (const output of outputs) {
    for (const chunkOrAsset of output.output) {
      if (chunkOrAsset.type === 'chunk') {
        moduleIds.push(...chunkOrAsset.moduleIds);
        files.push({ fileName: chunkOrAsset.fileName, code: chunkOrAsset.code });
      } else if (
        chunkOrAsset.type === 'asset' &&
        typeof chunkOrAsset.source === 'string' &&
        // Sourcemaps embed each source file's full original text (sourcesContent), including
        // code a flag compiled out of the actual bundle, so they'd falsely flag code that
        // never runs. Only the code that is actually emitted and executed matters here.
        !chunkOrAsset.fileName.endsWith('.map')
      ) {
        files.push({ fileName: chunkOrAsset.fileName, code: chunkOrAsset.source });
      }
    }
  }
  return { moduleIds, files };
}

/** Entry module every real build must include; if it's missing, the build was empty or this
 * script misread it, and every other check in this file would pass vacuously. */
const ENTRY_MODULE_SUFFIX = '/src/index.tsx';

async function main() {
  process.env.FLAGS_PROFILE = 'release';
  const { moduleIds, files } = await buildReleaseModuleGraph();

  if (moduleIds.length === 0 || !moduleIds.some(id => id.endsWith(ENTRY_MODULE_SUFFIX))) {
    console.error(
      `FAIL: the release build produced no modules (or is missing ${ENTRY_MODULE_SUFFIX}); ` +
        'this check cannot tell anything from an empty or misread build.'
    );
    process.exitCode = 1;
    return;
  }

  const findings = [...findModuleFindings(moduleIds), ...findTextFindings(files)];
  const allowlist = loadAllowlist();
  const unallowed = unallowedFindings(findings, allowlist);
  const stale = staleAllowlistEntries(findings, allowlist);

  if (unallowed.length > 0 || stale.length > 0) {
    if (unallowed.length > 0) {
      console.error('FAIL: the release bundle contains content that looks fake or unfinished:\n');
      for (const finding of unallowed) {
        console.error(`  - ${describe(finding)}`);
      }
      console.error(
        '\nEither remove it from the release build (gate it behind a build flag, see ' +
          'docs/build-flags.md), or add an entry to frontend/scripts/nothing-fake-allowlist.json ' +
          'with a reason and the PR that will remove it.'
      );
    }
    if (stale.length > 0) {
      console.error(
        (unallowed.length > 0 ? '\n' : '') +
          'FAIL: these frontend/scripts/nothing-fake-allowlist.json entries no longer match ' +
          'anything in the release build — remove them (that\'s the allowlist "shrinking to ' +
          'empty", not a bug in this check):\n'
      );
      for (const entry of stale) {
        console.error(`  - ${entry.type}: "${entry.pattern}" (${entry.reason})`);
      }
    }
    process.exitCode = 1;
    return;
  }

  const excused = findings.length;
  console.log(
    `ok: release bundle has no unexpected fake/placeholder content` +
      (excused > 0 ? ` (${excused} allowlisted finding(s))` : '')
  );
}

if (import.meta.url === pathToFileURL(realpathSync(process.argv[1])).href) {
  main().catch(err => {
    console.error(err);
    process.exitCode = 1;
  });
}
