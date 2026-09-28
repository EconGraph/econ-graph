// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { test } from '@playwright/test';

// The stack has no Keycloak yet. When the auth area's dev realm (AUTH-3) and the backend's OIDC
// verification (AUTH-5) land, the stack starts Keycloak with that realm and its seeded users,
// and the auth area's release spec (AUTH-10) replaces this file.
test.describe('auth', () => {
  test.skip(true, 'No Keycloak in the release stack until AUTH-3 (dev realm) and AUTH-5 (OIDC) land');

  test('a seeded user signs in through Keycloak', async () => {});
});
