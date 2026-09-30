/**
 * REQUIREMENT: Sign-in through Keycloak (OpenID Connect).
 * PURPOSE: Guard KEYCLOAK_SESSION_CHANGED against drifting from oidc-client-ts's own wording.
 * oidc-client-ts is pinned to an exact version (package.json), so this only needs to catch a
 * deliberate upgrade, not react to a range resolving differently between installs.
 */

import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

import { vi } from 'vitest';

// setupTests.vitest.ts stubs this module by default.
vi.unmock('../AuthContext');

import { KEYCLOAK_SESSION_CHANGED } from '../AuthContext';

const require = createRequire(import.meta.url);

describe('KEYCLOAK_SESSION_CHANGED', () => {
  it('matches every claim-mismatch message _validateIdTokenAttributes can throw', () => {
    // 'oidc-client-ts's package.json only exports the UMD build's path; the ESM build sits
    // alongside it and has the same source, so it is read straight from the resolved package.
    const umdPath = require.resolve('oidc-client-ts');
    const source = readFileSync(umdPath.replace('/dist/umd/', '/dist/esm/'), { encoding: 'utf-8' });
    // Every message this function throws, in the library's own source.
    const start = source.indexOf('_validateIdTokenAttributes(response, existingToken, nonce) {');
    const end = source.indexOf('\n  }', start);
    const body = source.slice(start, end);
    const thrown = [...body.matchAll(/new Error\("([^"]+)"\)/g)].map(m => m[1]);
    // "ID Token is missing a subject claim" is a malformed-token error, not a session change.
    const claimMismatches = thrown.filter(message => message !== 'ID Token is missing a subject claim');

    expect(claimMismatches.length).toBeGreaterThan(0);
    expect(new Set(KEYCLOAK_SESSION_CHANGED)).toEqual(new Set(claimMismatches));
  });
});
