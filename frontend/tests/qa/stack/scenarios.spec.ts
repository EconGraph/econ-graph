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

// How long the runner gives a stopped scenario to clean up before killing it
// (QA_TIMEOUT_GRACE_SECONDS), and what is left of the test's timeout after that to attach the
// output.
const GRACE_SECONDS = 45;
const STOP_MARGIN_MS = (GRACE_SECONDS + 30) * 1000;

// The scenarios this stack must pass, not skip: a regression that turns one into a skip fails.
const REQUIRED = ['annotation-round-trip', 'anonymous-api', 'anonymous-browser'];

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
  env.QA_TIMEOUT_GRACE_SECONDS = String(GRACE_SECONDS);
  // This stack is the ci profile's target.
  env.QA_LAB_TARGET_OK ??= new URL(target).hostname;

  // The runner owns its scenarios: on SIGTERM it passes the signal to the running scenario,
  // which cleans up (deletes its annotation, restores catalog roles), kills it after the grace
  // period, and exits. So stopping the runner is enough, here and if this worker exits early.
  const runner = spawn(
    resolve(repo, 'scripts/qa/run-scenarios.sh'),
    ['--target', target, '--profile', 'ci', '--json', report, '--require-pass', REQUIRED.join(',')],
    { cwd: repo, env, stdio: ['ignore', 'pipe', 'pipe'] }
  );
  let output = '';
  runner.stdout.on('data', chunk => (output += chunk));
  runner.stderr.on('data', chunk => (output += chunk));
  const stopRunner = () => {
    if (runner.exitCode === null && runner.signalCode === null) runner.kill('SIGTERM');
  };
  process.once('exit', stopRunner);

  let timedOut = false;
  const status = await new Promise<number | null>((done, failed) => {
    // No deadline when Playwright has none (--timeout=0, --debug).
    const deadline =
      testInfo.timeout > 0
        ? setTimeout(
            () => {
              timedOut = true;
              stopRunner();
            },
            Math.max(testInfo.timeout - STOP_MARGIN_MS, 1_000)
          )
        : undefined;
    runner.on('error', error => {
      clearTimeout(deadline);
      failed(error);
    });
    runner.on('close', code => {
      clearTimeout(deadline);
      done(code);
    });
  });
  process.off('exit', stopRunner);

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
  expect(status, 'the runner exit status (1: a scenario failed or a required one did not pass)').toBe(0);
});
