#!/usr/bin/env node
// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

// Gates `npm audit` (moderate severity and above, production and dev dependencies alike) while
// allowing specific, documented exceptions for advisories with no upstream fix. Plain
// `npm audit --audit-level moderate` has no per-advisory allowlist, so a single unfixable
// advisory (e.g. a transitive devDependency with no patched release) would otherwise force
// either a permanent CI failure or omitting a whole class of dependencies (--omit=dev) from the
// audit, which would hide any *other* future advisory in that class too. This checks every
// advisory individually against npm-audit-allowlist.json instead.
//
// Each allowlist entry needs a reason and a reviewBy date; ECO-394 is the first entry (sprintf-js,
// GHSA-hp3w-g68c-fv3c). The allowlist must shrink to empty as upstream fixes land — a stale entry
// (nothing in the current audit matches it) fails the check so it gets removed promptly.
//
// Shared between frontend/ and admin-frontend/ (ECO-395) — independent npm packages, each with
// its own package-lock.json and its own allowlist, so `--dir` and `--allowlist` point this one
// script at either. With neither flag it audits the current directory against the allowlist next
// to this script (frontend's own behavior).

import { execFileSync } from 'node:child_process';
import { readFileSync, realpathSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const defaultAllowlistPath = join(here, 'npm-audit-allowlist.json');

const GATED_SEVERITIES = new Set(['moderate', 'high', 'critical']);
const GHSA_URL_RE = /github\.com\/advisories\/(GHSA-[a-z0-9-]+)/i;

/** Loads the allowlist and rejects a malformed entry: a missing reason/reviewBy defeats the
 * point of the list, and an id that isn't a real GHSA id can't match anything on purpose. */
export function loadAllowlist(path = defaultAllowlistPath) {
  const allowlist = JSON.parse(readFileSync(path, 'utf8'));
  for (const entry of allowlist) {
    for (const field of ['id', 'package', 'reason', 'reviewBy']) {
      if (typeof entry[field] !== 'string' || entry[field].trim() === '') {
        throw new Error(
          `${path}: allowlist entry ${JSON.stringify(entry)} is missing a non-empty "${field}"`
        );
      }
    }
    if (!/^GHSA-/.test(entry.id)) {
      throw new Error(`${path}: allowlist entry id "${entry.id}" is not a GHSA id`);
    }
  }
  return allowlist;
}

/**
 * Extracts the direct advisories (not transitive "depends on a vulnerable version of X" edges)
 * from an `npm audit --json` report, at or above moderate severity. `report.vulnerabilities` is
 * keyed by package name; `via` holds either advisory objects (this package IS the vulnerable
 * one) or plain strings (this package merely depends on another vulnerable package, which has
 * its own entry with the real advisory object — so plain-string `via` entries are skipped here
 * to avoid listing the same advisory twice).
 */
export function findAdvisories(report) {
  const advisories = [];
  for (const pkgReport of Object.values(report.vulnerabilities ?? {})) {
    for (const via of pkgReport.via) {
      if (typeof via !== 'object' || via === null) continue;
      if (!GATED_SEVERITIES.has(via.severity)) continue;
      const match = GHSA_URL_RE.exec(via.url ?? '');
      advisories.push({
        id: match ? match[1] : via.url || via.title,
        package: pkgReport.name,
        severity: via.severity,
        title: via.title,
        url: via.url,
      });
    }
  }
  return advisories;
}

/** An allowlist entry excuses an advisory only when both its id and its package match — an
 * entry with the right id but the wrong package (a typo, or a GHSA id npm reused across
 * unrelated packages) must not silently suppress a real advisory. */
function matches(entry, advisory) {
  return entry.id === advisory.id && entry.package === advisory.package;
}

export function unallowedAdvisories(advisories, allowlist) {
  return advisories.filter(a => !allowlist.some(entry => matches(entry, a)));
}

/** Allowlist entries that excused nothing in this audit — stale, and due for removal (the fix
 * that resolved the advisory should have deleted the entry in the same PR). */
export function staleAllowlistEntries(advisories, allowlist) {
  return allowlist.filter(entry => !advisories.some(a => matches(entry, a)));
}

function runAudit(cwd) {
  try {
    const stdout = execFileSync('npm', ['audit', '--json'], { cwd, encoding: 'utf8' });
    return parseAuditReport(stdout);
  } catch (err) {
    // npm audit exits non-zero whenever it finds vulnerabilities; its JSON report is still on
    // stdout in that case, which is exactly what this check needs to parse.
    if (err.stdout) return parseAuditReport(err.stdout);
    throw err;
  }
}

/** npm itself (not just "audit found vulnerabilities") can fail with its JSON report replaced
 * by an `{"error": {...}}` document on stdout (bad registry, corrupt lockfile, no package-lock,
 * etc). Treating that as "zero advisories found" would silently pass with the allowlist
 * unexercised, or even mark every allowlist entry "stale" — fail loudly instead. */
export function parseAuditReport(stdout) {
  const report = JSON.parse(stdout);
  if (report.error) {
    throw new Error(
      `npm audit reported an error: ${report.error.summary ?? JSON.stringify(report.error)}`
    );
  }
  if (typeof report.vulnerabilities !== 'object' || report.vulnerabilities === null) {
    throw new Error('npm audit --json output has no "vulnerabilities" object');
  }
  return report;
}

/** Parses `[--dir <path>] [--allowlist <path>] [<dir>]`: `--dir`/a bare positional pick the
 * directory to audit (defaulting to the current directory), `--allowlist` picks the allowlist
 * file (defaulting to the one next to this script). */
export function parseArgs(argv) {
  let dir;
  let allowlistArg;
  const positional = [];
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--dir') dir = argv[++i];
    else if (argv[i] === '--allowlist') allowlistArg = argv[++i];
    else positional.push(argv[i]);
  }
  return { dir: dir ?? positional[0] ?? '.', allowlistArg };
}

function main() {
  const { dir, allowlistArg } = parseArgs(process.argv.slice(2));
  const cwd = resolve(dir);
  const allowlistPath = allowlistArg ? resolve(allowlistArg) : defaultAllowlistPath;
  const report = runAudit(cwd);
  const allowlist = loadAllowlist(allowlistPath);
  const advisories = findAdvisories(report);
  const unallowed = unallowedAdvisories(advisories, allowlist);
  const stale = staleAllowlistEntries(advisories, allowlist);

  if (unallowed.length > 0 || stale.length > 0) {
    if (unallowed.length > 0) {
      console.error('FAIL: npm audit found advisories that are not on the allowlist:\n');
      for (const a of unallowed) {
        console.error(`  - ${a.severity}: ${a.package} — ${a.title} (${a.id})`);
      }
      console.error(
        '\nEither fix it (upgrade, or an `overrides` entry pinning a patched version), or add an ' +
          `entry to ${allowlistPath} with a reason and review date if no fix exists upstream.`
      );
    }
    if (stale.length > 0) {
      console.error(
        (unallowed.length > 0 ? '\n' : '') +
          `FAIL: these ${allowlistPath} entries no longer match any advisory in this audit — ` +
          'remove them (that\'s the allowlist "shrinking to empty", not a bug in this check):\n'
      );
      for (const entry of stale) {
        console.error(`  - ${entry.id} (${entry.package}): ${entry.reason}`);
      }
    }
    process.exitCode = 1;
    return;
  }

  const excused = advisories.length;
  console.log(
    `ok: npm audit has no unallowed moderate+ advisories` +
      (excused > 0 ? ` (${excused} allowlisted advisory(ies))` : '')
  );
}

if (import.meta.url === pathToFileURL(realpathSync(process.argv[1])).href) {
  main();
}
