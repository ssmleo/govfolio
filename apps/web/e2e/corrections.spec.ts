// Public corrections log (goal 070b) against the REAL seeded API: the page
// reads corrected records straight from the shared record grammar
// (verification_state=corrected) and links each to the original it supersedes.
import { expect, test } from "@playwright/test";

import type { RecordDetail } from "./api";
import { apiGet } from "./api";
import { seedCorrection } from "./corrections-db";

test("lists a corrected record and links to it and the original it supersedes", async ({
  page,
}) => {
  const { originalId, correctedId } = await seedCorrection();

  // Sanity: promote's shape — a corrected row superseding the original, the
  // original untouched (supersede, never update).
  const detail = await apiGet<RecordDetail>(`/v1/records/${correctedId}`);
  expect(detail.record.verification_state).toBe("corrected");
  expect(detail.record.supersedes_record_id).toBe(originalId);

  await page.goto("/corrections");

  const entry = page.locator('[data-testid="correction-entry"]', {
    has: page.locator(`a[href="/r/${correctedId}"]`),
  });
  await expect(entry).toBeVisible();

  // Links to the earlier record it supersedes (the original stays on file).
  await expect(entry.getByTestId("superseded-link")).toHaveAttribute(
    "href",
    `/r/${originalId}`,
  );
  // Honest, text-labeled corrected badge.
  await expect(entry.getByText("Corrected", { exact: true })).toBeVisible();

  // What changed, at a glance: the declared band before -> after.
  await expect(entry.locator(".diff-before")).toContainText("$1,001");
  await expect(entry.locator(".diff-after")).toContainText("$50,000");

  // The correction links through to its own record page (full history there).
  await entry.locator(`a[href="/r/${correctedId}"]`).first().click();
  await page.waitForURL(`**/r/${correctedId}`);
  await expect(
    page.getByRole("heading", { level: 2, name: /Correction history/ }),
  ).toBeVisible();
});

test("corrections is linked from the site chrome and the sitemap", async ({
  page,
  request,
}) => {
  await page.goto("/");
  await expect(page.locator('a[href="/corrections"]').first()).toBeVisible();

  const sitemap = await request.get("/sitemaps/jurisdictions.xml");
  expect(sitemap.status()).toBe(200);
  expect(await sitemap.text()).toContain("/corrections</loc>");
});
