// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

// The `location.hash =` assignment below runs inside page.evaluate, in the browser.
/* global location */

import { expect, test, type Page } from '@playwright/test';

import { clickAndObserve, listControls } from './controls';

// Regression for the flake seen on #210 ("controls on /"): a click's navigation can commit
// after clickAndObserve's URL checks but before its final observableState(page) call, so that
// evaluate throws "Execution context was destroyed" instead of clickAndObserve returning an
// outcome. Reproducing the exact timing naturally is flaky by nature, so these force the same
// failure Chromium reports and check clickAndObserve recovers instead of throwing.

/**
 * Makes `page.evaluate`'s 2nd call (the post-click observableState, after the 1st, pre-click
 * one) throw the way Chromium does when a frame commits mid-evaluate, running `onSecondCall`
 * (also through the real `evaluate`) first. Every other call behaves normally.
 * @param page - The test's page.
 * @param onSecondCall - Run via the real `page.evaluate` right before the forced throw.
 * @returns How many times `page.evaluate` was called.
 */
function forceContextDestroyedOnSecondEvaluate(
  page: Page,
  onSecondCall?: () => void
): { calls: () => number } {
  const realEvaluate = page.evaluate.bind(page);
  let calls = 0;
  (page as unknown as { evaluate: (...args: unknown[]) => Promise<unknown> }).evaluate = async (
    ...args: unknown[]
  ) => {
    calls++;
    if (calls === 2) {
      if (onSecondCall) await (realEvaluate as (fn: () => void) => Promise<void>)(onSecondCall);
      throw new Error('Execution context was destroyed, most likely because of a navigation.');
    }
    return (realEvaluate as (...args: unknown[]) => Promise<unknown>)(...args);
  };
  return { calls: () => calls };
}

test.describe('clickAndObserve survives a context-destroyed evaluate', () => {
  test('recovers by retrying observableState when the URL is unchanged', async ({ page }) => {
    await page.setContent('<!doctype html><html><body><input type="checkbox" /></body></html>');
    const [control] = (await listControls(page)).filter(c => c.role === 'checkbox');
    if (!control) throw new Error('expected a checkbox control on the page');

    // Nothing navigates here, so the recovery path retries observableState instead of routing
    // through navigatedOutcome.
    const evaluate = forceContextDestroyedOnSecondEvaluate(page);

    const outcome = await clickAndObserve(page, control, () => true);

    expect(evaluate.calls(), 'observableState ran before the click, the forced throw, and the retry').toBe(3);
    expect(outcome).toEqual({ worked: true, disturbed: true, detail: 'changed the page' });
  });

  test('recovers by reporting a navigation when the URL changed during the forced error', async ({
    page,
  }) => {
    await page.setContent('<!doctype html><html><body><button>Go</button></body></html>');
    const [control] = (await listControls(page)).filter(c => c.role === 'button');
    if (!control) throw new Error('expected a button control on the page');
    const beforeUrl = page.url();

    // The click itself does nothing (a plain button), so clickAndObserve's own URL check passes
    // through to the post-click observableState call same as the first test. Only there, right
    // as the forced error fires, does the URL actually move -- simulating a navigation that
    // commits in that same window, landing after the check but before the evaluate call. The
    // context survives (this is a hash change, not a real navigation), so the recovery path
    // should go through navigatedOutcome rather than retrying observableState.
    const evaluate = forceContextDestroyedOnSecondEvaluate(page, () => {
      location.hash = 'forced-during-recovery';
    });

    const outcome = await clickAndObserve(page, control, () => true);

    expect(page.url()).not.toBe(beforeUrl);
    expect(evaluate.calls(), 'observableState ran before the click and the forced throw only').toBe(2);
    expect(outcome).toEqual({
      worked: true,
      disturbed: true,
      detail: `navigated to ${page.url()}`,
    });
  });
});
