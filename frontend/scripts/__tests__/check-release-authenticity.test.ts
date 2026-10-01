// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { describe, expect, it } from 'vitest';

import {
  findModuleFindings,
  findTextFindings,
  loadAllowlist,
  MODULE_PATTERNS,
  staleAllowlistEntries,
  TEXT_MARKERS,
  unallowedFindings,
} from '../check-release-authenticity.mjs';

describe('findModuleFindings', () => {
  it('flags a sample data module reachable in the build', () => {
    const findings = findModuleFindings(['/some/checkout/frontend/src/data/sampleCountryData.ts']);
    expect(findings).toHaveLength(1);
    expect(findings[0].type).toBe('module');
    expect(findings[0].label).toBe('sample/demo data module');
    expect(findings[0].key.endsWith('src/data/sampleCountryData.ts')).toBe(true);
  });

  it('flags a mock fixture module reachable in the build', () => {
    const findings = findModuleFindings([
      '/some/checkout/frontend/src/test-utils/mocks/graphql/financial-queries.ts',
    ]);
    expect(findings).toHaveLength(1);
    expect(findings[0].label).toBe('mock fixture module');
  });

  it('does not flag ordinary application or real-data modules', () => {
    const findings = findModuleFindings([
      '/some/checkout/frontend/src/pages/Dashboard.tsx',
      '/some/checkout/frontend/src/data/countrySeries.ts',
      '/some/checkout/frontend/src/components/global/InteractiveWorldMap.tsx',
    ]);
    expect(findings).toEqual([]);
  });

  it('flags a sample data module under a samples/ directory, not just a top-level file', () => {
    const findings = findModuleFindings([
      '/some/checkout/frontend/src/data/samples/countries.json',
    ]);
    expect(findings).toHaveLength(1);
    expect(findings[0].label).toBe('sample/demo data module');
  });

  it('every MODULE_PATTERNS entry has a label', () => {
    for (const pattern of MODULE_PATTERNS) {
      expect(pattern.label).toBeTruthy();
    }
  });
});

describe('findTextFindings', () => {
  it('flags a literal marker found in emitted code', () => {
    const files = [{ fileName: 'assets/main.js', code: 'x("Advanced Impact Analysis Coming Soon")' }];
    const findings = findTextFindings(files);
    expect(findings).toEqual([
      { type: 'text', key: 'Coming Soon', label: 'literal "Coming Soon"', files: ['assets/main.js'] },
    ]);
  });

  it('is case-sensitive and does not flag unrelated text', () => {
    const files = [{ fileName: 'assets/main.js', code: 'const x = "hello world";' }];
    expect(findTextFindings(files)).toEqual([]);
  });

  it('reports every file a marker appears in', () => {
    const files = [
      { fileName: 'assets/a.js', code: 'SAMPLE_RATE' },
      { fileName: 'assets/b.js', code: 'no markers here' },
      { fileName: 'assets/c.js', code: 'SAMPLE_SIZE' },
    ];
    const findings = findTextFindings(files, ['SAMPLE_']);
    expect(findings).toEqual([
      {
        type: 'text',
        key: 'SAMPLE_',
        label: 'literal "SAMPLE_"',
        files: ['assets/a.js', 'assets/c.js'],
      },
    ]);
  });

  it('every TEXT_MARKERS entry is a non-empty string', () => {
    for (const marker of TEXT_MARKERS) {
      expect(typeof marker).toBe('string');
      expect(marker.length).toBeGreaterThan(0);
    }
  });
});

describe('unallowedFindings', () => {
  const finding = { type: 'module', key: 'src/data/sampleCountryData.ts', label: 'sample/demo data module' };

  it('excuses a finding whose key exactly matches an allowlisted pattern of the same type', () => {
    const allowlist = [
      { type: 'module', pattern: 'src/data/sampleCountryData.ts', reason: 'x', removeBy: 'y' },
    ];
    expect(unallowedFindings([finding], allowlist)).toEqual([]);
  });

  it('does not excuse a finding whose key only contains the pattern as a substring', () => {
    const allowlist = [{ type: 'module', pattern: 'sampleCountryData.ts', reason: 'x', removeBy: 'y' }];
    expect(unallowedFindings([finding], allowlist)).toEqual([finding]);
  });

  it('does not excuse a finding of a different type even with a matching pattern string', () => {
    const allowlist = [
      { type: 'text', pattern: 'src/data/sampleCountryData.ts', reason: 'x', removeBy: 'y' },
    ];
    expect(unallowedFindings([finding], allowlist)).toEqual([finding]);
  });

  it('does not excuse a finding the allowlist does not mention', () => {
    expect(unallowedFindings([finding], [])).toEqual([finding]);
  });

  it('leaves an unrelated allowlist entry without masking other findings', () => {
    const other = { type: 'text', key: 'Coming Soon', label: 'literal "Coming Soon"', files: [] };
    const allowlist = [
      { type: 'module', pattern: 'src/data/sampleCountryData.ts', reason: 'x', removeBy: 'y' },
    ];
    expect(unallowedFindings([finding, other], allowlist)).toEqual([other]);
  });
});

describe('staleAllowlistEntries', () => {
  it('flags an entry whose pattern matches no current finding', () => {
    const allowlist = [{ type: 'module', pattern: 'src/data/longGone.ts', reason: 'x', removeBy: 'y' }];
    expect(staleAllowlistEntries([], allowlist)).toEqual(allowlist);
  });

  it('does not flag an entry that still excuses a finding', () => {
    const finding = { type: 'module', key: 'src/data/sampleCountryData.ts', label: 'x' };
    const allowlist = [
      { type: 'module', pattern: 'src/data/sampleCountryData.ts', reason: 'x', removeBy: 'y' },
    ];
    expect(staleAllowlistEntries([finding], allowlist)).toEqual([]);
  });
});

describe('loadAllowlist', () => {
  function writeAllowlist(entries: unknown) {
    const path = join(tmpdir(), `allowlist-${Math.random().toString(36).slice(2)}.json`);
    writeFileSync(path, JSON.stringify(entries));
    return path;
  }

  it('loads a well-formed allowlist', () => {
    const path = writeAllowlist([
      { type: 'module', pattern: 'src/data/sampleCountryData.ts', reason: 'x', removeBy: 'y' },
    ]);
    expect(loadAllowlist(path)).toHaveLength(1);
  });

  it('rejects an entry with an empty pattern (it would excuse everything of its type)', () => {
    const path = writeAllowlist([{ type: 'module', pattern: '', reason: 'x', removeBy: 'y' }]);
    expect(() => loadAllowlist(path)).toThrow(/pattern/);
  });

  it('rejects an entry missing a reason or removeBy', () => {
    const path = writeAllowlist([{ type: 'module', pattern: 'x', removeBy: 'y' }]);
    expect(() => loadAllowlist(path)).toThrow(/reason/);
  });
});
