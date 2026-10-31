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
// Each allowlist entry names an advisory (`id`), the vulnerable `package`, the `versions` it
// excuses (a range, see parseRange), optionally the exact installed copies (`paths`, lockfile
// keys such as "node_modules/minimatch"), a `reason` and a `reviewBy` date (YYYY-MM-DD). An
// advisory is excused only when every installed copy of its package that npm reports is covered
// by an entry: its version is in the entry's range and, when the entry lists paths, its path is
// one of them. So an entry for one old copy never excuses a different vulnerable copy (ECO-810).
// Installed versions come from the audited directory's package-lock.json. The check fails on an
// entry whose reviewBy has passed, and on a stale entry (one that covers nothing in the current
// audit), so the allowlist shrinks to empty as upstream fixes land.
//
// Shared between frontend/ and admin-frontend/ (ECO-395) — independent npm packages, each with
// its own package-lock.json and its own allowlist, so `--dir` and `--allowlist` point this one
// script at either. With neither flag it audits the current directory against the allowlist next
// to this script (frontend's own behavior).

import { execFileSync } from 'node:child_process';
import { validateMinimatchCompatibility } from './check-minimatch-compat.mjs';
import { existsSync, readFileSync, realpathSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const defaultAllowlistPath = join(here, 'npm-audit-allowlist.json');

const GATED_SEVERITIES = new Set(['moderate', 'high', 'critical']);
const GHSA_URL_RE = /github\.com\/advisories\/(GHSA-[a-z0-9-]+)/i;

const VERSION_RE = /^(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?$/;
const COMPARATOR_RE = /^(>=|<=|>|<|=)?(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)$/;

/** `[major, minor, patch, prerelease identifiers]` of a plain semver version, or null. */
function parseVersion(version) {
  const m = VERSION_RE.exec(String(version ?? '').trim());
  if (!m) return null;
  return [Number(m[1]), Number(m[2]), Number(m[3]), m[4] ? m[4].split('.') : []];
}

/** Semver precedence of two parsed versions (-1, 0 or 1), prerelease included. */
function compareVersions(a, b) {
  for (let i = 0; i < 3; i++) {
    if (a[i] !== b[i]) return a[i] < b[i] ? -1 : 1;
  }
  const [pa, pb] = [a[3], b[3]];
  if (pa.length === 0 || pb.length === 0) return pa.length === pb.length ? 0 : pa.length === 0 ? 1 : -1;
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    if (pa[i] === undefined) return -1;
    if (pb[i] === undefined) return 1;
    const [na, nb] = [/^\d+$/.test(pa[i]), /^\d+$/.test(pb[i])];
    if (na && nb && Number(pa[i]) !== Number(pb[i])) return Number(pa[i]) < Number(pb[i]) ? -1 : 1;
    if (na !== nb) return na ? -1 : 1;
    if (!na && pa[i] !== pb[i]) return pa[i] < pb[i] ? -1 : 1;
  }
  return 0;
}

/**
 * Parses an allowlist `versions` range: comparator sets joined by `||`, each a space-separated
 * list of `1.2.3`, `=1.2.3`, `>1.2.3`, `>=1.2.3`, `<1.2.3` or `<=1.2.3` that must all hold.
 * Deliberately narrower than npm's syntax (no `^`, `~`, `x` or `*`): an allowlist range should
 * say exactly which versions it excuses. Throws on anything else.
 */
export function parseRange(range) {
  if (typeof range !== 'string' || range.trim() === '') throw new Error('empty version range');
  return range.split('||').map(set => {
    const comparators = set.trim().split(/\s+/).filter(Boolean);
    if (comparators.length === 0) throw new Error(`empty comparator set in "${range}"`);
    return comparators.map(c => {
      const m = COMPARATOR_RE.exec(c);
      if (!m) throw new Error(`unsupported comparator "${c}" in "${range}"`);
      return { op: m[1] ?? '=', version: parseVersion(m[2]) };
    });
  });
}

/** Whether `version` (a plain semver string) is in `range` (see parseRange). As in npm's
 * semver, a prerelease version is in a comparator set only if some comparator in it names a
 * prerelease of the same major.minor.patch: `>=3.0.0 <4.0.0` does not take `4.0.0-alpha.1`, and
 * `<=3.1.5` does not take `3.1.5-rc.1`. */
export function satisfies(version, range) {
  const v = parseVersion(version);
  if (!v) return false;
  const sameRelease = bound => bound[0] === v[0] && bound[1] === v[1] && bound[2] === v[2];
  return parseRange(range).some(
    set =>
      set.every(({ op, version: bound }) => {
        const c = compareVersions(v, bound);
        return { '=': c === 0, '>': c > 0, '>=': c >= 0, '<': c < 0, '<=': c <= 0 }[op];
      }) &&
      (v[3].length === 0 || set.some(({ version: bound }) => bound[3].length > 0 && sameRelease(bound)))
  );
}

/** Loads the allowlist and rejects a malformed entry: a missing reason, reviewBy or versions
 * defeats the point of the list, an id that isn't a real GHSA id can't match anything on
 * purpose, and a range or path it can't read must not excuse more than intended. */
export function loadAllowlist(path = defaultAllowlistPath) {
  const allowlist = JSON.parse(readFileSync(path, 'utf8'));
  for (const entry of allowlist) {
    for (const field of ['id', 'package', 'versions', 'reason', 'reviewBy']) {
      if (typeof entry[field] !== 'string' || entry[field].trim() === '') {
        throw new Error(
          `${path}: allowlist entry ${JSON.stringify(entry)} is missing a non-empty "${field}"`
        );
      }
    }
    if (!/^GHSA-/.test(entry.id)) {
      throw new Error(`${path}: allowlist entry id "${entry.id}" is not a GHSA id`);
    }
    try {
      parseRange(entry.versions);
    } catch (err) {
      throw new Error(`${path}: allowlist entry ${entry.id} (${entry.package}): ${err.message}`);
    }
    // Round-tripped, since Date.parse rolls an impossible day over (2027-02-30 to 2027-03-02).
    const reviewBy = new Date(`${entry.reviewBy}T00:00:00Z`);
    if (
      !/^\d{4}-\d{2}-\d{2}$/.test(entry.reviewBy) ||
      Number.isNaN(reviewBy.getTime()) ||
      reviewBy.toISOString().slice(0, 10) !== entry.reviewBy
    ) {
      throw new Error(`${path}: allowlist entry ${entry.id} reviewBy "${entry.reviewBy}" is not YYYY-MM-DD`);
    }
    if (
      entry.paths !== undefined &&
      (!Array.isArray(entry.paths) ||
        entry.paths.length === 0 ||
        !entry.paths.every(p => typeof p === 'string' && /^node_modules\//.test(p)))
    ) {
      throw new Error(
        `${path}: allowlist entry ${entry.id} "paths" must be a non-empty list of lockfile paths ` +
          '(e.g. "node_modules/minimatch")'
      );
    }
  }
  return allowlist;
}

/** Entries whose reviewBy date is before `today` (YYYY-MM-DD): overdue for review. */
export function overdueAllowlistEntries(allowlist, today = new Date().toISOString().slice(0, 10)) {
  return allowlist.filter(entry => entry.reviewBy < today);
}

/** The `packages` map of a package-lock.json (lockfile path -> {version, ...}), or {} without
 * one (every copy's version is then unknown, and nothing is excused). */
export function loadLockPackages(dir) {
  const path = join(dir, 'package-lock.json');
  if (!existsSync(path)) return {};
  return JSON.parse(readFileSync(path, 'utf8')).packages ?? {};
}

/**
 * Extracts the direct advisories (not transitive "depends on a vulnerable version of X" edges)
 * from an `npm audit --json` report, at or above moderate severity. `report.vulnerabilities` is
 * keyed by package name; `via` holds either advisory objects (this package IS the vulnerable
 * one) or plain strings (this package merely depends on another vulnerable package, which has
 * its own entry with the real advisory object — so plain-string `via` entries are skipped here
 * to avoid listing the same advisory twice).
 */
export function findAdvisories(report, lockPackages = {}) {
  const advisories = [];
  for (const pkgReport of Object.values(report.vulnerabilities ?? {})) {
    // Every installed copy npm reports for the package, with its version from the lockfile
    // (null when the lockfile doesn't say: such a copy is never excused). npm lists only the
    // copies in the package's vulnerable range, so each counts as affected by each advisory.
    const copies = (pkgReport.nodes ?? []).map(path => ({
      path,
      version: lockPackages[path]?.version ?? null,
    }));
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
        copies,
      });
    }
  }
  return advisories;
}

/** An allowlist entry covers one installed copy of an advisory's package only when the id and
 * package match (a GHSA id npm reused across unrelated packages must not suppress a real
 * advisory), the copy's version is in the entry's range, and, if the entry lists paths, the
 * copy's path is one of them. */
function covers(entry, advisory, copy) {
  return (
    entry.id === advisory.id &&
    entry.package === advisory.package &&
    copy.version !== null &&
    satisfies(copy.version, entry.versions) &&
    (entry.paths === undefined || entry.paths.includes(copy.path))
  );
}

/** The copies of `advisory` no allowlist entry covers. An advisory npm reported without any
 * copy is uncovered as a whole: there is nothing an entry could have been checked against. */
function uncoveredCopies(advisory, allowlist) {
  if (advisory.copies.length === 0) return [{ path: '(none reported)', version: null }];
  return advisory.copies.filter(copy => !allowlist.some(entry => covers(entry, advisory, copy)));
}

/** Advisories with at least one copy no entry covers, each with those copies. */
export function unallowedAdvisories(advisories, allowlist) {
  return advisories
    .map(a => ({ ...a, uncovered: uncoveredCopies(a, allowlist) }))
    .filter(a => a.uncovered.length > 0);
}

/** Allowlist entries that covered nothing in this audit — stale, and due for removal (the fix
 * that resolved the advisory should have deleted the entry in the same PR). */
export function staleAllowlistEntries(advisories, allowlist) {
  return allowlist.filter(
    entry => !advisories.some(a => a.copies.some(copy => covers(entry, a, copy)))
  );
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
 * file (defaulting to the one next to this script). A later `--dir` wins over an earlier bare
 * positional, so a caller can't be overridden by an unexpected positional slipping in first. */
export function parseArgs(argv) {
  let dir;
  let allowlistArg;
  const positional = [];
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === '--dir' || argv[i] === '--allowlist') {
      // A following flag (e.g. `--dir --allowlist admin-frontend`) is never a legitimate path —
      // treat it the same as a missing value instead of swallowing the next flag as this one's
      // argument.
      if (i + 1 >= argv.length || argv[i + 1].startsWith('-')) {
        throw new Error(`${argv[i]} needs a value`);
      }
      if (argv[i] === '--dir') dir = argv[++i];
      else allowlistArg = argv[++i];
    } else if (argv[i].startsWith('--')) {
      throw new Error(`unexpected argument: ${argv[i]}`);
    } else {
      positional.push(argv[i]);
    }
  }
  if (positional.length > 1) {
    throw new Error(`unexpected argument: ${positional[1]}`);
  }
  return { dir: dir ?? positional[0] ?? '.', allowlistArg };
}

function main() {
  const { dir, allowlistArg } = parseArgs(process.argv.slice(2));
  const cwd = resolve(dir);
  const allowlistPath = allowlistArg ? resolve(allowlistArg) : defaultAllowlistPath;
  const allowlistDisplayPath = relative(process.cwd(), allowlistPath);
  validateMinimatchCompatibility(resolve(here, '..'));
  const report = runAudit(cwd);
  const allowlist = loadAllowlist(allowlistPath);
  const advisories = findAdvisories(report, loadLockPackages(cwd));
  const unallowed = unallowedAdvisories(advisories, allowlist);
  const stale = staleAllowlistEntries(advisories, allowlist);
  const overdue = overdueAllowlistEntries(allowlist);

  if (unallowed.length > 0 || stale.length > 0 || overdue.length > 0) {
    if (unallowed.length > 0) {
      console.error('FAIL: npm audit found advisories that are not on the allowlist:\n');
      for (const a of unallowed) {
        console.error(`  - ${a.severity}: ${a.package} — ${a.title} (${a.id})`);
        for (const copy of a.uncovered) {
          console.error(`      not covered: ${copy.path} ${copy.version ?? '(version unknown)'}`);
        }
      }
      console.error(
        '\nEither fix it (upgrade, or an `overrides` entry pinning a patched version), or add an ' +
          `entry to ${allowlistDisplayPath} with a reason and review date if no fix exists upstream.`
      );
    }
    if (stale.length > 0) {
      console.error(
        (unallowed.length > 0 ? '\n' : '') +
          `FAIL: these ${allowlistDisplayPath} entries no longer match any advisory in this audit ` +
          '— remove them (that\'s the allowlist "shrinking to empty", not a bug in this check):\n'
      );
      for (const entry of stale) {
        console.error(`  - ${entry.id} (${entry.package} ${entry.versions}): ${entry.reason}`);
      }
    }
    if (overdue.length > 0) {
      console.error(
        (unallowed.length > 0 || stale.length > 0 ? '\n' : '') +
          `FAIL: these ${allowlistDisplayPath} entries are past their reviewBy date — check ` +
          'whether a fix exists now, then remove the entry or set a new date with a reason:\n'
      );
      for (const entry of overdue) {
        console.error(`  - ${entry.id} (${entry.package} ${entry.versions}): reviewBy ${entry.reviewBy}`);
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
