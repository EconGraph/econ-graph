// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

import { spawn } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { expect, test } from '@playwright/test';

// Runs the QA scenario suite (scripts/qa/run-scenarios.sh) with the ci profile against the
// release end-to-end stack that playwright.qa-stack.config.ts starts. The suite's own JSON report
// goes to QA_REPORT (default frontend/qa-scenarios-report.json) and is attached here.

const repo = resolve(import.meta.dirname, '../../../..');

// Left of the test's timeout once the runner is stopped: enough to attach what it wrote.
const ATTACH_MARGIN_MS = 60_000;

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

  // Its own process group (detached), so a runner that outlives the deadline is stopped with
  // every scenario it started, and the output is still attached.
  const runner = spawn(
    resolve(repo, 'scripts/qa/run-scenarios.sh'),
    ['--target', target, '--profile', 'ci', '--json', report],
    { cwd: repo, env, stdio: ['ignore', 'pipe', 'pipe'], detached: true }
  );
  let output = '';
  runner.stdout.on('data', chunk => (output += chunk));
  runner.stderr.on('data', chunk => (output += chunk));
  const stopGroup = (signal: NodeJS.Signals) => {
    try {
      if (runner.pid) process.kill(-runner.pid, signal);
    } catch {
      // Already gone.
    }
  };

  let timedOut = false;
  const status = await new Promise<number | null>((done, failed) => {
    const deadline = setTimeout(
      () => {
        timedOut = true;
        // SIGTERM first: the runner's scenarios clean up what they created.
        stopGroup('SIGTERM');
        setTimeout(() => stopGroup('SIGKILL'), ATTACH_MARGIN_MS / 2).unref();
      },
      Math.max(testInfo.timeout - ATTACH_MARGIN_MS, 1_000)
    );
    runner.on('error', error => {
      clearTimeout(deadline);
      failed(error);
    });
    runner.on('close', code => {
      clearTimeout(deadline);
      done(code);
    });
  });

  console.log(output);
  await testInfo.attach('qa-scenarios-output.txt', { body: output, contentType: 'text/plain' });
  try {
    await testInfo.attach('qa-scenarios-report.json', {
      body: readFileSync(report),
      contentType: 'application/json',
    });
  } catch {
    // No report: the runner refused or was stopped before writing one; its output says why.
  }
  expect(timedOut, 'the runner finished before the test deadline').toBe(false);
  expect(status, 'the runner exit status (1: a scenario failed)').toBe(0);
});
