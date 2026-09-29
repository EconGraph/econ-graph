// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

/**
 * Controls the every-control crawl (every-control.spec.ts) lets through although clicking them
 * changes nothing a visitor can see, or navigates to a page that doesn't work. Every entry needs a
 * reason; the fix is to make the control work or remove it, then delete its entry. The crawl
 * reports an entry that excused nothing in a run as a `stale allowlist entry` annotation, without
 * failing, so the PR that fixes a control doesn't have to touch this file. Release 1 ships with it as short as possible (plan review F9/A).
 */
export interface AllowlistEntry {
  /** The route's `path` as App.tsx writes it (e.g. '/series/:id'), or 'layout' for the header and sidebar. */
  route: string;
  role: 'link' | 'button' | 'tab' | 'checkbox' | 'switch' | 'combobox';
  /** The control's accessible name, exactly. */
  name: string;
  /** Why clicking it legitimately changes nothing, or which issue fixes it. */
  reason: string;
}

export const ALLOWLIST: readonly AllowlistEntry[] = [
  {
    route: '/',
    role: 'button',
    name: 'Employment Data',
    reason: 'Opens /explore?category=..., which the explorer ignores. ECO-246.',
  },
  {
    route: '/',
    role: 'button',
    name: 'Inflation Indicators',
    reason: 'Opens /explore?category=..., which the explorer ignores. ECO-246.',
  },
  {
    route: '/',
    role: 'button',
    name: 'GDP & Growth',
    reason: 'Opens /explore?category=..., which the explorer ignores. ECO-246.',
  },
  {
    route: '/',
    role: 'button',
    name: 'refresh data',
    reason: 'Recent Data Releases refresh icon has no handler. ECO-246.',
  },
  {
    route: '/',
    role: 'button',
    name: 'view details',
    reason: 'Recent Data Releases row icons have no handler. ECO-246.',
  },
];
