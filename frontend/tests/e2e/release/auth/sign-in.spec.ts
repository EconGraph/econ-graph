// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { expect, test } from '@playwright/test';

// The stack now starts Keycloak with the auth area's dev realm (AUTH-3) and the backend verifies
// its tokens (AUTH-5), wired into docker-compose.release-e2e.yml and playwright.release.config.ts
// by REL-2b. There is no sign-in UI yet (AUTH-7, AUTH-9), so this checks the infra directly: a
// token from the dev realm's password grant is accepted by the backend and identifies a caller.
// AUTH-10 replaces this file with a real sign-in-through-the-UI spec once AUTH-9 lands.
const keycloakUrl = 'http://localhost:8081';

async function tokenFor(request: import('@playwright/test').APIRequestContext, user: string) {
  const response = await request.post(
    `${keycloakUrl}/realms/econ-graph/protocol/openid-connect/token`,
    {
      form: {
        grant_type: 'password',
        client_id: 'econ-graph-web',
        scope: 'openid',
        username: user,
        password: `${user}-dev-password`,
      },
    }
  );
  expect(response.ok()).toBe(true);
  const body = await response.json();
  return body.access_token as string;
}

test.describe('auth', () => {
  test('an anonymous caller is rejected from a sign-in-only query', async ({ request }) => {
    const response = await request.post('/graphql', {
      data: {
        query: '{ chartCollaborators(chartId: "00000000-0000-0000-0000-000000000000") { id } }',
      },
    });
    expect(response.ok()).toBe(true);
    const body = await response.json();
    expect(body.errors?.[0]?.message).toBe('Authentication required');
  });

  test('a seeded user signs in through Keycloak and the backend accepts the token', async ({
    request,
  }) => {
    const token = await tokenFor(request, 'alice');

    const response = await request.post('/graphql', {
      headers: { Authorization: `Bearer ${token}` },
      data: {
        query: '{ chartCollaborators(chartId: "00000000-0000-0000-0000-000000000000") { id } }',
      },
    });
    expect(response.ok()).toBe(true);
    const body = await response.json();
    expect(body.errors).toBeUndefined();
    expect(body.data.chartCollaborators).toEqual([]);
  });
});
