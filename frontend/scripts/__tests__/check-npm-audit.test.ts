// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { describe, expect, it } from 'vitest';

import {
  findAdvisories,
  loadAllowlist,
  loadLockPackages,
  overdueAllowlistEntries,
  parseArgs,
  parseAuditReport,
  parseRange,
  satisfies,
  staleAllowlistEntries,
  unallowedAdvisories,
} from '../check-npm-audit.mjs';

function advisory(name: string, severity: string, url: string, title = 'vulnerable') {
  return { name, via: [{ severity, url, title }] };
}

describe('findAdvisories', () => {
  it('extracts a direct advisory at or above moderate severity', () => {
    const report = {
      vulnerabilities: {
        'sprintf-js': advisory(
          'sprintf-js',
          'moderate',
          'https://github.com/advisories/GHSA-hp3w-g68c-fv3c'
        ),
      },
    };
    const advisories = findAdvisories(report);
    expect(advisories).toHaveLength(1);
    expect(advisories[0]).toMatchObject({
      id: 'GHSA-hp3w-g68c-fv3c',
      package: 'sprintf-js',
      severity: 'moderate',
    });
  });

  it('ignores transitive "depends on a vulnerable version of X" edges (plain-string via)', () => {
    const report = {
      vulnerabilities: {
        argparse: { name: 'argparse', via: ['sprintf-js'] },
        'sprintf-js': advisory(
          'sprintf-js',
          'moderate',
          'https://github.com/advisories/GHSA-hp3w-g68c-fv3c'
        ),
      },
    };
    expect(findAdvisories(report)).toHaveLength(1);
  });

  it('ignores advisories below moderate severity', () => {
    const report = {
      vulnerabilities: {
        'low-sev-pkg': advisory('low-sev-pkg', 'low', 'https://github.com/advisories/GHSA-aaaa'),
      },
    };
    expect(findAdvisories(report)).toEqual([]);
  });

  it('falls back to the advisory title when no GHSA id is in the url', () => {
    const report = {
      vulnerabilities: {
        'some-pkg': advisory('some-pkg', 'high', '', 'some vulnerability with no url'),
      },
    };
    const advisories = findAdvisories(report);
    expect(advisories[0].id).toBe('some vulnerability with no url');
  });

  it('extracts every advisory object when a package has more than one', () => {
    const report = {
      vulnerabilities: {
        'multi-advisory-pkg': {
          name: 'multi-advisory-pkg',
          via: [
            {
              severity: 'moderate',
              url: 'https://github.com/advisories/GHSA-1111-1111-1111',
              title: 'first issue',
            },
            {
              severity: 'high',
              url: 'https://github.com/advisories/GHSA-2222-2222-2222',
              title: 'second issue',
            },
          ],
        },
      },
    };
    const advisories = findAdvisories(report);
    expect(advisories).toHaveLength(2);
    expect(advisories.map(a => a.id)).toEqual(['GHSA-1111-1111-1111', 'GHSA-2222-2222-2222']);
  });
});

const MINIMATCH = 'GHSA-g3hv-75wg-m249';

/** An `npm audit --json` report with one minimatch advisory over `nodes`, and a lockfile
 * `packages` map giving each node's installed version. */
function minimatchAudit(copies: Record<string, string | undefined>) {
  const report = {
    vulnerabilities: {
      minimatch: {
        name: 'minimatch',
        nodes: Object.keys(copies),
        via: [
          {
            severity: 'moderate',
            url: `https://github.com/advisories/${MINIMATCH}`,
            title: 'ReDoS',
          },
        ],
      },
    },
  };
  const lock = Object.fromEntries(
    Object.entries(copies).flatMap(([path, version]) => (version ? [[path, { version }]] : []))
  );
  return findAdvisories(report, lock);
}

const minimatch315 = {
  id: MINIMATCH,
  package: 'minimatch',
  versions: '3.1.5',
  paths: ['node_modules/minimatch'],
  reason: 'test',
  reviewBy: '2099-01-01',
};

describe('findAdvisories copies', () => {
  it('attaches every reported copy with its lockfile version, null when the lockfile lacks it', () => {
    const [a] = minimatchAudit({
      'node_modules/minimatch': '3.1.5',
      'node_modules/nyc/node_modules/minimatch': undefined,
    });
    expect(a.copies).toEqual([
      { path: 'node_modules/minimatch', version: '3.1.5' },
      { path: 'node_modules/nyc/node_modules/minimatch', version: null },
    ]);
  });
});

describe('unallowedAdvisories', () => {
  it('excuses an advisory when every copy is in an entry\'s range and paths', () => {
    const advisories = minimatchAudit({ 'node_modules/minimatch': '3.1.5' });
    expect(unallowedAdvisories(advisories, [minimatch315])).toEqual([]);
  });

  it('fails an out-of-range copy of an allowlisted package (ECO-810)', () => {
    // The 3.1.5 entry must not excuse a vulnerable minimatch 5, 7 or 8 that appears later.
    for (const version of ['5.1.6', '7.4.6', '8.0.4', '3.1.4', '3.1.6']) {
      const advisories = minimatchAudit({ 'node_modules/minimatch': version });
      const [failed] = unallowedAdvisories(advisories, [minimatch315]);
      expect(failed?.uncovered, version).toEqual([{ path: 'node_modules/minimatch', version }]);
    }
  });

  it('fails a second copy even when the allowlisted copy is still there', () => {
    const advisories = minimatchAudit({
      'node_modules/minimatch': '3.1.5',
      'node_modules/glob/node_modules/minimatch': '5.1.6',
    });
    const [failed] = unallowedAdvisories(advisories, [minimatch315]);
    expect(failed.uncovered).toEqual([
      { path: 'node_modules/glob/node_modules/minimatch', version: '5.1.6' },
    ]);
  });

  it('fails an in-range copy at a path the entry does not list', () => {
    const advisories = minimatchAudit({ 'node_modules/nyc/node_modules/minimatch': '3.1.5' });
    expect(unallowedAdvisories(advisories, [minimatch315])).toHaveLength(1);
    const anyPath = { ...minimatch315, paths: undefined };
    expect(unallowedAdvisories(advisories, [anyPath])).toEqual([]);
  });

  it('fails a copy whose version the lockfile does not give', () => {
    const advisories = minimatchAudit({ 'node_modules/minimatch': undefined });
    expect(unallowedAdvisories(advisories, [{ ...minimatch315, versions: '>=0.0.0' }])).toHaveLength(1);
  });

  it('fails an advisory npm reported without any copy', () => {
    const advisories = minimatchAudit({});
    expect(unallowedAdvisories(advisories, [minimatch315])[0].uncovered).toEqual([
      { path: '(none reported)', version: null },
    ]);
  });

  it('does not excuse an advisory whose id matches but whose package does not', () => {
    // A GHSA id allowlisted for one package must not suppress the same id surfacing against a
    // different package (a typo in the allowlist, or an id npm reused across unrelated packages).
    const advisories = minimatchAudit({ 'node_modules/minimatch': '3.1.5' });
    expect(unallowedAdvisories(advisories, [{ ...minimatch315, package: 'glob' }])).toHaveLength(1);
    expect(unallowedAdvisories(advisories, [{ ...minimatch315, id: 'GHSA-aaaa-bbbb-cccc' }])).toHaveLength(1);
  });

  it('flags an advisory not on the allowlist at all', () => {
    const advisories = minimatchAudit({ 'node_modules/minimatch': '3.1.5' });
    expect(unallowedAdvisories(advisories, [])).toHaveLength(1);
  });
});

describe('staleAllowlistEntries', () => {
  it('flags an allowlist entry that excuses nothing in the current audit', () => {
    expect(staleAllowlistEntries([], [minimatch315])).toEqual([minimatch315]);
  });

  it('does not flag an allowlist entry that still covers a copy', () => {
    const advisories = minimatchAudit({ 'node_modules/minimatch': '3.1.5' });
    expect(staleAllowlistEntries(advisories, [minimatch315])).toEqual([]);
  });

  it('flags an entry whose version no longer matches any copy (the copy was upgraded)', () => {
    const advisories = minimatchAudit({ 'node_modules/minimatch': '5.1.6' });
    expect(staleAllowlistEntries(advisories, [minimatch315])).toEqual([minimatch315]);
  });

  it('flags an allowlist entry whose id matches but whose package does not', () => {
    const advisories = minimatchAudit({ 'node_modules/minimatch': '3.1.5' });
    const other = { ...minimatch315, package: 'glob' };
    expect(staleAllowlistEntries(advisories, [other])).toEqual([other]);
  });
});

describe('version ranges', () => {
  it('matches exact versions, comparator sets and ||', () => {
    expect(satisfies('3.1.5', '3.1.5')).toBe(true);
    expect(satisfies('3.1.5', '=3.1.5')).toBe(true);
    expect(satisfies('3.1.6', '3.1.5')).toBe(false);
    expect(satisfies('3.1.9', '>=3.1.5 <3.2.0')).toBe(true);
    expect(satisfies('3.2.0', '>=3.1.5 <3.2.0')).toBe(false);
    expect(satisfies('9.0.1', '3.1.5 || >9.0.0')).toBe(true);
    expect(satisfies('10.0.0', '<=9.9.9')).toBe(false);
  });

  it('orders prereleases below their release', () => {
    expect(satisfies('4.0.0-rc.1', '<4.0.0')).toBe(true);
    expect(satisfies('4.0.0-rc.2', '>4.0.0-rc.10')).toBe(false);
    expect(satisfies('4.0.0-beta', '<4.0.0-rc.1')).toBe(true);
  });

  it('never matches a version it cannot read', () => {
    expect(satisfies('latest', '>=0.0.0')).toBe(false);
    expect(satisfies('', '>=0.0.0')).toBe(false);
  });

  it('refuses npm syntax wider than an allowlist should be', () => {
    for (const range of ['^3.1.5', '~3.1.5', '3.x', '*', '3.1', '', '3.1.5 ||', '>= 3.1.5']) {
      expect(() => parseRange(range), range).toThrow();
    }
  });
});

describe('loadAllowlist', () => {
  function load(entries: unknown[]) {
    const dir = mkdtempSync(join(tmpdir(), 'npm-audit-allowlist-'));
    try {
      const path = join(dir, 'allowlist.json');
      writeFileSync(path, JSON.stringify(entries));
      return loadAllowlist(path);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  }

  it('loads a complete entry', () => {
    expect(load([minimatch315])).toEqual([minimatch315]);
  });

  it('refuses an entry without versions, with a range it cannot read, or with bad paths', () => {
    const { versions: _versions, ...noVersions } = minimatch315;
    expect(() => load([noVersions])).toThrow(/"versions"/);
    expect(() => load([{ ...minimatch315, versions: '^3.1.5' }])).toThrow(/unsupported comparator/);
    for (const paths of [[], 'node_modules/minimatch', ['minimatch'], [7]]) {
      expect(() => load([{ ...minimatch315, paths }]), JSON.stringify(paths)).toThrow(/"paths"/);
    }
  });

  it('refuses a reviewBy that is not a YYYY-MM-DD date', () => {
    for (const reviewBy of ['2027-13-45', 'next year', '2027/01/31']) {
      expect(() => load([{ ...minimatch315, reviewBy }]), reviewBy).toThrow(/reviewBy/);
    }
  });
});

describe('overdueAllowlistEntries', () => {
  it('flags an entry whose reviewBy is before today, not on or after it', () => {
    const entry = { ...minimatch315, reviewBy: '2026-10-30' };
    expect(overdueAllowlistEntries([entry], '2026-10-31')).toEqual([entry]);
    expect(overdueAllowlistEntries([entry], '2026-10-30')).toEqual([]);
    expect(overdueAllowlistEntries([entry], '2026-10-29')).toEqual([]);
  });
});

describe('the checked-in allowlists', () => {
  it('load, and every entry pins its versions', () => {
    for (const path of ['scripts/npm-audit-allowlist.json', '../admin-frontend/scripts/npm-audit-allowlist.json']) {
      for (const entry of loadAllowlist(path)) {
        expect(parseRange(entry.versions).length, `${path} ${entry.id}`).toBeGreaterThan(0);
      }
    }
  });

  it('cover exactly the versions this lockfile installs at their paths', () => {
    const lock = loadLockPackages('.');
    for (const entry of loadAllowlist('scripts/npm-audit-allowlist.json')) {
      for (const path of entry.paths ?? []) {
        expect(lock[path]?.version, `${entry.id} ${path}`).toBeDefined();
        expect(satisfies(lock[path].version, entry.versions), `${entry.id} ${path}`).toBe(true);
      }
    }
  });
});

describe('parseAuditReport', () => {
  it('parses a normal report with a vulnerabilities object', () => {
    const report = parseAuditReport(JSON.stringify({ vulnerabilities: {} }));
    expect(report.vulnerabilities).toEqual({});
  });

  it('throws instead of silently treating an npm error document as "no vulnerabilities"', () => {
    // npm itself can fail (bad registry, corrupt lockfile) and still exit non-zero with a JSON
    // document on stdout, but shaped as `{"error": {...}}` instead of a real audit report. If
    // this were treated as a zero-advisory report, every allowlist entry would look "stale" and
    // get removed, and once the allowlist is empty a network failure would print "ok" and pass.
    const errorDoc = JSON.stringify({ error: { code: 'ENOLOCK', summary: 'no package-lock.json found' } });
    expect(() => parseAuditReport(errorDoc)).toThrow(/no package-lock\.json found/);
  });

  it('throws on a report with no vulnerabilities object at all', () => {
    expect(() => parseAuditReport(JSON.stringify({ auditReportVersion: 2 }))).toThrow(
      /vulnerabilities/
    );
  });
});

describe('parseArgs', () => {
  // This script is shared between frontend/ and admin-frontend/ (ECO-395): admin-frontend
  // invokes it with --dir/--allowlist pointed at its own directory and allowlist file.
  it('defaults to the current directory and no allowlist override when given nothing', () => {
    expect(parseArgs([])).toEqual({ dir: '.', allowlistArg: undefined });
  });

  it('takes a bare positional argument as the directory (frontend/admin-frontend\'s own call)', () => {
    expect(parseArgs(['.'])).toEqual({ dir: '.', allowlistArg: undefined });
  });

  it('takes --dir and --allowlist for another package (admin-frontend calling this copy)', () => {
    expect(parseArgs(['--dir', 'admin-frontend', '--allowlist', 'admin-frontend/scripts/npm-audit-allowlist.json'])).toEqual({
      dir: 'admin-frontend',
      allowlistArg: 'admin-frontend/scripts/npm-audit-allowlist.json',
    });
  });

  it('--dir overrides a bare positional directory', () => {
    expect(parseArgs(['.', '--dir', 'admin-frontend'])).toEqual({
      dir: 'admin-frontend',
      allowlistArg: undefined,
    });
  });

  it('throws on --dir with no value, rather than silently falling back to "."', () => {
    expect(() => parseArgs(['--dir'])).toThrow(/--dir needs a value/);
  });

  it('throws on --allowlist with no value, rather than silently using the default allowlist', () => {
    expect(() => parseArgs(['--allowlist'])).toThrow(/--allowlist needs a value/);
  });

  it('throws when a flag is immediately followed by another flag instead of a value', () => {
    // --dir --allowlist admin-frontend must not take "--allowlist" as the directory.
    expect(() => parseArgs(['--dir', '--allowlist', 'admin-frontend'])).toThrow(
      /--dir needs a value/
    );
  });

  it('throws on an unrecognized flag instead of treating it as a directory', () => {
    expect(() => parseArgs(['--bogus', '.'])).toThrow(/unexpected argument: --bogus/);
  });

  it('throws on an unexpected second positional argument', () => {
    expect(() => parseArgs(['a', 'b'])).toThrow(/unexpected argument: b/);
  });
});
