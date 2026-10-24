// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { expect, test, type Page } from '@playwright/test';

import { signInOnKeycloak } from '../auth/helpers';
import { OIDC_ISSUER, SKIP_AUTH, USERS } from '../env';

// ECO-264: when sign-in, its callback or a session restore fails, the page says it is signed
// out (or why sign-in failed) rather than half signed in, and there is a way to sign in again.
// Signed in as USERS.failureSignIn (frank), whom no other spec signs in, and serial, so frank
// never signs in twice at once. Live QA: runs as qa-bob against a deployed build (env.ts).
test.describe('sign-in failures', () => {
  test.skip(SKIP_AUTH, 'RELEASE_SKIP_AUTH=1: sign-in is known broken for this run');
  test.slow();
  test.describe.configure({ mode: 'serial' });

  const AFTER_SIGN_IN = { timeout: 15_000 };
  const issuer = OIDC_ISSUER.replace(/\/+$/, '');
  const signInButton = (page: Page) => page.getByRole('button', { name: 'Sign in', exact: true });
  const userMenu = (page: Page) => page.getByRole('button', { name: 'user menu' });

  async function expectSignedIn(page: Page) {
    await expect(userMenu(page)).toBeVisible(AFTER_SIGN_IN);
    await userMenu(page).click();
    await expect(page.getByRole('menu').getByText(USERS.failureSignIn.displayName)).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(signInButton(page)).toHaveCount(0);
  }

  test('an unreachable sign-in service is reported, and signing in works once it is back', async ({
    page,
  }) => {
    const discovery = `${issuer}/.well-known/openid-configuration`;
    await page.route(discovery, route => route.abort('connectionrefused'));
    await page.goto('/about');
    await signInButton(page).click();

    await expect(
      page.getByRole('alert').filter({ hasText: 'Could not reach the sign-in service' })
    ).toBeVisible();
    await expect(page).toHaveURL(/\/about$/);
    await expect(userMenu(page)).toHaveCount(0);

    await page.unroute(discovery);
    await signInButton(page).click();
    await signInOnKeycloak(page, USERS.failureSignIn);
    await expect(page).toHaveURL(/\/about$/, AFTER_SIGN_IN);
    await expectSignedIn(page);
  });

  test('a failed callback says sign-in did not complete, signed out, and signing in again works', async ({
    page,
  }) => {
    const tokenEndpoint = `${issuer}/protocol/openid-connect/token`;
    await page.route(tokenEndpoint, route =>
      route.fulfill({
        status: 500,
        json: { error: 'server_error', error_description: 'Injected failure (ECO-264)' },
      })
    );
    await page.goto('/about');
    await signInButton(page).click();
    await signInOnKeycloak(page, USERS.failureSignIn);

    await expect(page.getByText(/Sign-in did not complete/)).toBeVisible(AFTER_SIGN_IN);
    await expect(userMenu(page)).toHaveCount(0);

    await page.unroute(tokenEndpoint);
    await page.getByRole('button', { name: 'Back to the dashboard' }).click();
    await expect(page).toHaveURL(/\/$/);
    await expect(signInButton(page)).toBeVisible(AFTER_SIGN_IN);
    // Keycloak's own session survived the failed exchange, so it signs straight back in.
    await signInButton(page).click();
    await expectSignedIn(page);
  });

  test('a session that ended at the identity provider loads signed out, and can sign in again', async ({
    page,
    context,
  }) => {
    await page.goto('/about');
    await signInButton(page).click();
    await signInOnKeycloak(page, USERS.failureSignIn);
    await expectSignedIn(page);

    // Ends the Keycloak session as far as this browser can tell (as an expired or revoked SSO
    // session would). Tokens live in memory, so the reload has to restore the session from it.
    await context.clearCookies();
    await page.reload();

    await expect(signInButton(page)).toBeVisible(AFTER_SIGN_IN);
    await expect(userMenu(page)).toHaveCount(0);

    await signInButton(page).click();
    await signInOnKeycloak(page, USERS.failureSignIn);
    await expect(page).toHaveURL(/\/about$/, AFTER_SIGN_IN);
    await expectSignedIn(page);
  });
});
