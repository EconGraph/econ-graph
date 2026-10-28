// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { spawnSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { expect, test } from '@playwright/test';

// Runs the QA scenario suite (scripts/qa/run-scenarios.sh) with the ci profile against the
// release end-to-end stack that playwright.qa-stack.config.ts starts. The suite's own JSON report
// goes to QA_REPORT (default frontend/qa-scenarios-report.json) and is attached here.

const repo = resolve(import.meta.dirname, '../../../..');

test('QA scenario suite, ci profile, against the release e2e stack', async ({}, testInfo) => {
  const target = process.env.QA_STACK_TARGET ?? '';
  const report = process.env.QA_REPORT ?? resolve(repo, 'frontend/qa-scenarios-report.json');
  // The runner and its nested Playwright run get a clean environment: none of this run's own
  // Playwright worker settings.
  const env: Record<string, string> = {};
  for (const [name, value] of Object.entries(process.env)) {
    if (value !== undefined && !/^(PW_|PLAYWRIGHT_|TEST_)/.test(name)) env[name] = value;
  }
  env.QA_OIDC_ISSUER ??= process.env.QA_STACK_OIDC_ISSUER ?? '';

  const run = spawnSync(
    resolve(repo, 'scripts/qa/run-scenarios.sh'),
    ['--target', target, '--profile', 'ci', '--json', report],
    { cwd: repo, env, encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'], maxBuffer: 64 << 20 }
  );
  const output = `${run.stdout ?? ''}${run.stderr ?? ''}`;
  console.log(output);
  await testInfo.attach('qa-scenarios-output.txt', { body: output, contentType: 'text/plain' });
  try {
    await testInfo.attach('qa-scenarios-report.json', {
      body: readFileSync(report),
      contentType: 'application/json',
    });
  } catch {
    // No report: the runner refused before running anything; its output says why.
  }
  expect(run.error, 'the runner started').toBeUndefined();
  expect(run.status, 'the runner exit status (1: a scenario failed)').toBe(0);
});
