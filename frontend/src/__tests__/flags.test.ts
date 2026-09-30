import { mkdtempSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';

import { ESLint } from 'eslint';
import { afterEach, describe, expect, it } from 'vitest';

import { flagDefines, loadBuildFlags, resolveFlagProfile } from '../../flags.config.ts';

const frontendDir = resolve(import.meta.dirname, '../..');

function flagDir(files: Record<string, object>): string {
  const dir = mkdtempSync(join(tmpdir(), 'flags-test-'));
  for (const [profile, flags] of Object.entries(files)) {
    writeFileSync(join(dir, `${profile}.json`), JSON.stringify({ profile, flags }));
  }
  return dir;
}

describe('build flags in tests', () => {
  it('match the profile vitest runs with (dev unless FLAGS_PROFILE is set)', () => {
    const { build_canary } = loadBuildFlags(resolveFlagProfile('serve'));
    expect(__FLAGS__.build_canary).toBe(build_canary);
  });
});

describe('resolveFlagProfile', () => {
  const saved = process.env.FLAGS_PROFILE;
  afterEach(() => {
    if (saved === undefined) delete process.env.FLAGS_PROFILE;
    else process.env.FLAGS_PROFILE = saved;
  });

  it('defaults to release for builds and dev otherwise', () => {
    delete process.env.FLAGS_PROFILE;
    expect(resolveFlagProfile('build')).toBe('release');
    expect(resolveFlagProfile('serve')).toBe('dev');
  });

  it('lets FLAGS_PROFILE override, and rejects unknown profiles', () => {
    process.env.FLAGS_PROFILE = 'dev';
    expect(resolveFlagProfile('build')).toBe('dev');
    process.env.FLAGS_PROFILE = 'staging';
    expect(() => resolveFlagProfile('build')).toThrow(/FLAGS_PROFILE must be one of/);
  });
});

describe('loadBuildFlags', () => {
  it('reads the build flags of a profile and skips other kinds', () => {
    const dir = flagDir({
      release: { build_canary: { kind: 'build', value: false }, kill: { kind: 'ops', value: true } },
      dev: { build_canary: { kind: 'build', value: true }, kill: { kind: 'ops', value: true } },
    });
    expect(loadBuildFlags('release', dir)).toEqual({ build_canary: false });
    expect(loadBuildFlags('dev', dir)).toEqual({ build_canary: true });
    expect(flagDefines('dev', dir)).toEqual({ '__FLAGS__.build_canary': 'true' });
  });

  it('rejects a build flag that is not a boolean', () => {
    const dir = flagDir({ release: { build_canary: { kind: 'build', value: 'off' } } });
    expect(() => loadBuildFlags('release', dir)).toThrow(/is not a boolean/);
  });

  it('rejects a file generated for another profile', () => {
    const dir = mkdtempSync(join(tmpdir(), 'flags-test-'));
    writeFileSync(join(dir, 'release.json'), JSON.stringify({ profile: 'dev', flags: {} }));
    expect(() => loadBuildFlags('release', dir)).toThrow(/expected "release"/);
  });

  it('fails clearly when the flag file is missing', () => {
    const dir = mkdtempSync(join(tmpdir(), 'flags-test-'));
    expect(() => loadBuildFlags('release', dir)).toThrow(/run node scripts\/flags-merge/);
  });

  it('reads the repository flag files', () => {
    expect(loadBuildFlags('release')).toMatchObject({ build_canary: false });
    expect(loadBuildFlags('dev')).toMatchObject({ build_canary: true });
  });
});

describe('lint rules for build flags', () => {
  const eslint = new ESLint({ cwd: frontendDir, overrideConfigFile: 'eslint.config.js' });

  async function errors(code: string): Promise<string[]> {
    const [result] = await eslint.lintText(code, { filePath: join(frontendDir, 'src/probe.ts') });
    return result.messages.filter(m => m.severity === 2).map(m => m.ruleId ?? '');
  }

  it('forbids reading the flag files at runtime', async () => {
    expect(await errors("import f from '../../config/flags/flags.flagd.json';\nexport { f };\n"))
      .toContain('no-restricted-imports');
    expect(await errors("import r from '../flags/release.json';\nexport { r };\n")).toContain(
      'no-restricted-imports'
    );
    expect(await errors('export const p = fetch(`/config/flags/${name}.flagd.json`);\n')).toContain(
      'no-restricted-syntax'
    );
    expect(await errors("export const p = fetch('/config/flags/dev.flagd.json');\n")).toContain(
      'no-restricted-syntax'
    );
  });

  it('forbids using __FLAGS__ other than as __FLAGS__.<name>', async () => {
    expect(await errors('export const all = __FLAGS__;\n')).toContain('no-restricted-syntax');
    expect(await errors("export const one = __FLAGS__['build_canary'];\n")).toContain(
      'no-restricted-syntax'
    );
    expect(await errors('export const ok = __FLAGS__.build_canary;\n')).toEqual([]);
  });
});
