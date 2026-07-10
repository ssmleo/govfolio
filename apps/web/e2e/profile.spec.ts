import { expect, test } from "@playwright/test";

import type { PoliticianProfile } from "./api";
import { apiGet, seededRecords } from "./api";

test("politician profile shows identity, mandates, and a linked timeline", async ({
  page,
}) => {
  const records = await seededRecords();
  const politicianId = records[0]?.politician_id;
  expect(politicianId).toBeTruthy();
  const profile = await apiGet<PoliticianProfile>(`/v1/politicians/${politicianId}`);

  await page.goto(`/p/${profile.id}`);
  await expect(page.getByRole("heading", { level: 1 })).toHaveText(
    profile.canonical_name,
  );

  // Mandates (identity block)
  expect(profile.mandates.length).toBeGreaterThan(0);
  const firstMandate = profile.mandates[0];
  if (firstMandate) {
    await expect(page.locator(".mandates li").first()).toContainText(firstMandate.body);
  }

  // Record summary line
  await expect(page.getByText(/disclosure records? on file/)).toBeVisible();

  // Timeline rows exist and link through to record pages. The first <tr> in
  // each filing group is a group header (its own "View filing" link opens
  // the archived document in a new tab), so target record rows specifically.
  const rows = page.locator("table.records tbody tr.record-row");
  await expect(rows.first()).toBeVisible();
  await rows.first().locator("a").click();
  await expect(page).toHaveURL(/\/r\/[0-7][0-9A-HJKMNP-TV-Z]{25}$/);
  await expect(page.locator(".provenance")).toBeVisible();
});

test("unknown politician id renders the not-found page", async ({ page }) => {
  const response = await page.goto("/p/01AAAAAAAAAAAAAAAAAAAAAAAA");
  expect(response?.status()).toBe(404);
  await expect(page.getByRole("heading", { name: "Not found" })).toBeVisible();
});
