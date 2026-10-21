// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

/**
 * `__APP_VERSION__` for Vite's `define`: package.json's `version`, so the version the app shows
 * can't drift from the release (scripts/release/tag.sh checks package.json's version against the tag).
 */
export function appVersionDefine(dir: string = import.meta.dirname): Record<string, string> {
  const { version } = JSON.parse(readFileSync(resolve(dir, 'package.json'), 'utf8')) as {
    version: string;
  };
  return { __APP_VERSION__: JSON.stringify(version) };
}
