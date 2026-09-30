// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

// page.evaluate callbacks run in the browser.
/* global HTMLAnchorElement, HTMLTextAreaElement, HTMLSelectElement, getComputedStyle, location */

import { expect, type Frame, type Locator, type Page, type Response } from '@playwright/test';

/** The ARIA roles the crawl clicks: links, buttons, and the controls that behave like them. */
export const CONTROL_ROLES = ['link', 'button', 'tab', 'checkbox', 'switch', 'combobox'] as const;
export type ControlRole = (typeof CONTROL_ROLES)[number];

/** One visible, enabled control on a freshly loaded page. */
export interface Control {
  role: ControlRole;
  /** Accessible name, as Playwright's `getByRole(role, { name })` matches it; '' when unnamed. */
  name: string;
  /** Position among the visible controls of this role, to find it again after a reload. */
  index: number;
  /** In the header or sidebar (outside `main`), which every route shares. */
  inLayout: boolean;
  /** For a link: its resolved href. */
  href?: string;
  /**
   * Already the current choice: a selected tab, or a link to the page it is on (aria-current).
   * Clicking it is expected to do nothing.
   */
  isCurrent: boolean;
}

/**
 * Loads a route and waits for it to settle: its data requests answered and loading indicators
 * gone, so the controls listed are the ones a visitor would see.
 * @param page - The test's page.
 * @param path - The route, relative to baseURL.
 */
export async function gotoSettled(page: Page, path: string) {
  await page.goto(path);
  await settle(page);
}

async function settle(page: Page) {
  await page.waitForLoadState('networkidle', { timeout: 10_000 }).catch(() => undefined);
  await expect(page.locator('[role="progressbar"]:not([aria-valuenow]):visible')).toHaveCount(0, {
    timeout: 10_000,
  });
}

/**
 * The accessible name of a control, from its ARIA snapshot's first line (`- button "Sign in"`).
 * @param control - The control's locator.
 * @returns The name, or '' when the control has none.
 */
async function accessibleName(control: Locator): Promise<string> {
  const [first] = (await control.ariaSnapshot()).split('\n');
  // YAML single-quotes the line when the control also has text of its own:
  // `- 'button "Navigate to About: About EconGraph"': About About EconGraph`, with any ' doubled.
  const quoted = /^- '(.*)'(?::|$)/.exec(first);
  const line = quoted ? quoted[1].replace(/''/g, "'") : first;
  const match = /^(?:- )?[\w-]+ "((?:[^"\\]|\\.)*)"/.exec(line);
  return match ? JSON.parse(`"${match[1]}"`) : '';
}

/**
 * Lists the visible, enabled controls on the current page.
 * @param page - A settled page.
 * @returns The controls, in document order within each role.
 */
export async function listControls(page: Page): Promise<Control[]> {
  const controls: Control[] = [];
  for (const role of CONTROL_ROLES) {
    const all = page.getByRole(role).filter({ visible: true });
    const count = await all.count();
    for (let index = 0; index < count; index++) {
      const control = all.nth(index);
      if (await control.isDisabled()) continue;
      const { inLayout, href, isCurrent } = await control.evaluate(el => ({
        inLayout: !el.closest('main'),
        href: el instanceof HTMLAnchorElement && el.getAttribute('href') ? el.href : undefined,
        isCurrent:
          (el.getAttribute('role') === 'tab' && el.getAttribute('aria-selected') === 'true') ||
          ['page', 'true'].includes(el.getAttribute('aria-current') ?? ''),
      }));
      controls.push({
        role,
        name: await accessibleName(control),
        index,
        inLayout,
        href,
        isCurrent,
      });
    }
  }
  return controls;
}

/**
 * Finds a listed control again on a freshly loaded page.
 * @param page - The page, reloaded on the same route.
 * @param control - The control as `listControls` listed it.
 * @returns Its locator.
 */
export function locate(page: Page, control: Control): Locator {
  return page.getByRole(control.role).filter({ visible: true }).nth(control.index);
}

/**
 * What a visitor can observe on the page: the URL, the visible text, the state ARIA exposes
 * (expanded, pressed, selected, checked, field values), open overlays, the theme's colours,
 * the layout's geometry and the scroll position. A click that leaves all of these unchanged,
 * and starts no request, download or popup, did nothing.
 * @param page - The page.
 * @returns A string that differs whenever any of those differ.
 */
export async function observableState(page: Page): Promise<string> {
  return page.evaluate(() => {
    const states = Array.from(
      document.querySelectorAll(
        '[aria-expanded], [aria-pressed], [aria-selected], [aria-checked], input, textarea, select'
      )
    ).map(el =>
      [
        el.getAttribute('aria-expanded'),
        el.getAttribute('aria-pressed'),
        el.getAttribute('aria-selected'),
        el.getAttribute('aria-checked'),
        el instanceof HTMLInputElement ? `${el.checked}:${el.value}` : '',
        el instanceof HTMLTextAreaElement || el instanceof HTMLSelectElement ? el.value : '',
      ].join(',')
    );
    const overlays = document.querySelectorAll(
      '[role="dialog"], [role="alertdialog"], [role="menu"], [role="listbox"], [role="alert"]'
    ).length;
    const style = getComputedStyle(document.body);
    const geometry = Array.from(document.querySelectorAll('header, nav, main, aside')).map(el => {
      const r = el.getBoundingClientRect();
      return [r.left, r.top, r.width, r.height].map(Math.round).join(',');
    });
    return JSON.stringify([
      location.href,
      document.title,
      document.body.innerText,
      states,
      overlays,
      style.backgroundColor,
      style.color,
      geometry,
      Math.round(window.scrollY),
    ]);
  });
}

/** What clicking a control did. */
export interface ClickOutcome {
  /** The click did something a visitor can see, and any navigation landed somewhere real. */
  worked: boolean;
  /** The page may differ from a fresh load of the route, so the next click needs a reload. */
  disturbed: boolean;
  /** What it did, or why it counts as dead; for the report. */
  detail: string;
}

/**
 * Checks where a same-site navigation landed: a route the app declares, with every query
 * parameter the control navigated with still in the URL (a page that ignores a parameter drops
 * it), and something rendered in `main`.
 * @param page - The page after the navigation settled.
 * @param visited - Every URL the main frame navigated to during the click, in order.
 * @param isAppPath - Whether a pathname matches a route in App.tsx.
 * @returns Why the navigation is broken, or null when it is fine.
 */
async function brokenNavigation(
  page: Page,
  visited: URL[],
  isAppPath: (pathname: string) => boolean
): Promise<string | null> {
  const landed = new URL(page.url());
  if (!isAppPath(landed.pathname)) return `went to ${landed.pathname}, which no route serves`;
  for (const url of visited.filter(u => u.origin === landed.origin)) {
    for (const [key, value] of url.searchParams) {
      if (landed.searchParams.get(key) !== value) {
        return `went to ${url.pathname}${url.search}, and the page dropped ${key}=${value}`;
      }
    }
  }
  const main = page.getByRole('main');
  if ((await main.count()) === 1 && !(await main.innerText()).trim()) {
    return `went to ${landed.pathname}, which renders nothing`;
  }
  return null;
}

/**
 * Whether `err` is Playwright's error for a navigation destroying the JS context mid-`evaluate`.
 * @param err - The error a `page.evaluate` call rejected with.
 * @returns Whether it was a context-destroyed error rather than something else.
 */
function isContextDestroyed(err: unknown): boolean {
  return err instanceof Error && /[Ee]xecution context was destroyed/.test(err.message);
}

/**
 * Judges a click that landed on a different URL than it started on.
 * @param page - The page after the navigation settled.
 * @param beforeUrl - The URL before the click.
 * @param visited - Every URL the main frame navigated to during the click, in order.
 * @param isAppPath - Whether a pathname matches a route in App.tsx.
 * @returns The outcome.
 */
async function navigatedOutcome(
  page: Page,
  beforeUrl: string,
  visited: URL[],
  isAppPath: (pathname: string) => boolean
): Promise<ClickOutcome> {
  if (new URL(page.url()).origin !== new URL(beforeUrl).origin) {
    return { worked: true, disturbed: true, detail: `left for ${new URL(page.url()).origin}` };
  }
  const broken = await brokenNavigation(page, visited, isAppPath);
  return broken
    ? { worked: false, disturbed: true, detail: broken }
    : { worked: true, disturbed: true, detail: `navigated to ${page.url()}` };
}

/**
 * Clicks a control and reports whether it did anything a visitor can see: a navigation to a
 * working page of the app (or off the site), a successful request, a download or popup, or a
 * change to what `observableState` reads.
 * @param page - A settled page showing the control.
 * @param control - The control, as listed on a fresh load of this page.
 * @param isAppPath - Whether a pathname matches a route in App.tsx.
 * @returns The outcome.
 */
export async function clickAndObserve(
  page: Page,
  control: Control,
  isAppPath: (pathname: string) => boolean
): Promise<ClickOutcome> {
  const target = locate(page, control);
  // Found by position, so make sure it is still the same control: something else on the page
  // (another spec's rows in the shared database, say) may have shifted the list since.
  const name = (await target.count()) === 0 ? null : await accessibleName(target);
  if (name !== control.name) {
    return {
      worked: false,
      disturbed: true,
      detail: `the page changed between loads: found ${name === null ? 'nothing' : `"${name}"`} in its place`,
    };
  }
  const before = await observableState(page);
  const beforeUrl = page.url();

  const effects: string[] = [];
  const visited: URL[] = [];
  const onResponse = (response: Response) => {
    const request = response.request();
    if (['fetch', 'xhr'].includes(request.resourceType()) && response.ok()) {
      effects.push(`${request.method()} ${new URL(request.url()).pathname}`);
    }
  };
  const onNavigated = (frame: Frame) => {
    if (frame === page.mainFrame()) visited.push(new URL(frame.url()));
  };
  const onPopup = () => effects.push('opened a window');
  const onDownload = () => effects.push('started a download');
  page.on('response', onResponse);
  page.on('framenavigated', onNavigated);
  page.on('popup', onPopup);
  page.on('download', onDownload);
  try {
    await target.click({ timeout: 5_000 });
  } catch (err) {
    return {
      worked: false,
      disturbed: true,
      detail: `click failed: ${(err as Error).message.split('\n')[0]}`,
    };
  } finally {
    // Let the click's effects land, then take the pointer off the control so a hover tooltip
    // doesn't count as one.
    await page.mouse.move(0, 0);
    await page.waitForTimeout(400);
    await page.waitForLoadState('networkidle', { timeout: 5_000 }).catch(() => undefined);
    page.off('response', onResponse);
    page.off('framenavigated', onNavigated);
    page.off('popup', onPopup);
    page.off('download', onDownload);
  }

  if (page.url() !== beforeUrl) {
    return navigatedOutcome(page, beforeUrl, visited, isAppPath);
  }
  if (visited.length > 0) {
    // Navigated and came back to the same URL: a redirect may have dropped a parameter on the
    // way. A clean round trip still has to change something below, so a link that only reloads
    // the page it is on doesn't count as working.
    const broken = await brokenNavigation(page, visited, isAppPath);
    if (broken) return { worked: false, disturbed: true, detail: broken };
  }
  let after: string;
  try {
    after = await observableState(page);
  } catch (err) {
    if (!isContextDestroyed(err)) throw err;
    // The click's navigation committed after the URL checks above but before this evaluate ran,
    // destroying the context mid-call. Wait for it to finish, then judge it as a navigation
    // rather than crashing. The `framenavigated` listener is already detached by now, so
    // `visited` won't include this late navigation; navigatedOutcome's own route and empty-page
    // checks still apply.
    await page.waitForLoadState('load', { timeout: 10_000 }).catch(() => undefined);
    await settle(page);
    if (page.url() !== beforeUrl) {
      return navigatedOutcome(page, beforeUrl, visited, isAppPath);
    }
    after = await observableState(page);
  }
  if (after !== before) return { worked: true, disturbed: true, detail: 'changed the page' };
  if (effects.length > 0) return { worked: true, disturbed: true, detail: effects.join(', ') };
  return { worked: false, disturbed: visited.length > 0, detail: 'nothing changed' };
}
