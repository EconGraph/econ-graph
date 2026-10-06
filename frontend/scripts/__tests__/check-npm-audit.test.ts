// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { describe, expect, it } from 'vitest';

import {
  findAdvisories,
  parseAuditReport,
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

describe('unallowedAdvisories', () => {
  const allowlist = [{ id: 'GHSA-hp3w-g68c-fv3c', package: 'sprintf-js' }];

  it('excuses an advisory whose id is on the allowlist', () => {
    const advisories = [{ id: 'GHSA-hp3w-g68c-fv3c', package: 'sprintf-js', severity: 'moderate' }];
    expect(unallowedAdvisories(advisories, allowlist)).toEqual([]);
  });

  it('flags an advisory not on the allowlist', () => {
    const advisories = [{ id: 'GHSA-68fv-2mgg-jv7q', package: 'source-map-js', severity: 'high' }];
    expect(unallowedAdvisories(advisories, allowlist)).toEqual(advisories);
  });

  it('does not excuse an advisory whose id matches but whose package does not', () => {
    // A GHSA id allowlisted for one package must not suppress the same id surfacing against a
    // different package (a typo in the allowlist, or an id npm reused across unrelated packages).
    const advisories = [
      { id: 'GHSA-hp3w-g68c-fv3c', package: 'some-other-package', severity: 'moderate' },
    ];
    expect(unallowedAdvisories(advisories, allowlist)).toEqual(advisories);
  });
});

describe('staleAllowlistEntries', () => {
  it('flags an allowlist entry that excuses nothing in the current audit', () => {
    const allowlist = [{ id: 'GHSA-aaaa', package: 'fixed-pkg' }];
    expect(staleAllowlistEntries([], allowlist)).toEqual(allowlist);
  });

  it('does not flag an allowlist entry that still matches a current advisory', () => {
    const allowlist = [{ id: 'GHSA-hp3w-g68c-fv3c', package: 'sprintf-js' }];
    const advisories = [{ id: 'GHSA-hp3w-g68c-fv3c', package: 'sprintf-js', severity: 'moderate' }];
    expect(staleAllowlistEntries(advisories, allowlist)).toEqual([]);
  });

  it('flags an allowlist entry whose id matches but whose package does not', () => {
    const allowlist = [{ id: 'GHSA-hp3w-g68c-fv3c', package: 'sprintf-js' }];
    const advisories = [
      { id: 'GHSA-hp3w-g68c-fv3c', package: 'some-other-package', severity: 'moderate' },
    ];
    expect(staleAllowlistEntries(advisories, allowlist)).toEqual(allowlist);
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
