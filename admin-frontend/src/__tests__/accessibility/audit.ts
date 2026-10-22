import { mkdirSync, writeFileSync } from "node:fs";
import { act } from "@testing-library/react";
import { expect } from "vitest";
import axe from "axe-core";

// Preserve full results (including incomplete checks) before asserting, so
// failures retain actionable rule IDs, affected HTML and remediation advice.
export async function auditPage(name: string) {
  let results: axe.AxeResults;
  await act(async () => {
    results = await axe.run(document.body);
  });
  mkdirSync("accessibility-results", { recursive: true });
  writeFileSync(
    `accessibility-results/axe-${name}.json`,
    JSON.stringify(results!, null, 2),
  );
  expect(results!.passes.length).toBeGreaterThan(0);
  expect(results!.violations).toEqual([]);
}
