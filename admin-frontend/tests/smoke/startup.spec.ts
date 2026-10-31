import { expect, test } from "@playwright/test";

test("the production admin app starts without Node globals", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));

  // Keep startup deterministic without requiring a running backend.
  await page.route("**/graphql", (route) =>
    route.fulfill({ json: { data: {} } }),
  );

  await page.goto("/admin/");
  expect(await page.evaluate(() => typeof process)).toBe("undefined");
  await expect(
    page.getByRole("heading", { name: "EconGraph Crawler Administration" }),
  ).toBeVisible();
  await expect(
    page.getByRole("tab", { name: "Dashboard", exact: true }),
  ).toBeVisible();
  await page.getByRole("tab", { name: "Configuration", exact: true }).click();
  await expect(page.getByRole("tabpanel")).toBeVisible();
  expect(errors).toEqual([]);
});

test("expired operator credentials use the shared client login handling", async ({
  page,
}) => {
  await page.addInitScript(() => {
    if (location.pathname === "/admin/")
      localStorage.setItem("auth_token", "expired");
  });
  await page.route("**/graphql", (route) =>
    route.fulfill({ status: 401, json: { error: "unauthorized" } }),
  );
  await page.goto("/admin/");
  await expect(page).toHaveURL(/\/login$/);
  expect(
    await page.evaluate(() => localStorage.getItem("auth_token")),
  ).toBeNull();
});
