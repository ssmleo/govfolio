// Admin dashboard smoke flows (goal 091) against the REAL seeded API — the
// same server-side X-Admin-Token flow as the reviewer suite (see
// playwright.config.ts / e2e/api.ts), except every admin page reads the
// token server-side (lib/api.ts adminHeaders()) so no client-side auth is
// needed here; page.goto() alone exercises the real fetch.
import { expect, test } from "@playwright/test";

test("GET /admin renders without a client-side exception and shows a live stat tile", async ({
  page,
}) => {
  const pageErrors: Error[] = [];
  page.on("pageerror", (error) => pageErrors.push(error));

  const response = await page.goto("/admin");
  expect(response?.status()).toBe(200);
  await expect(page.getByRole("heading", { name: "Overview", level: 1 })).toBeVisible();

  // At least one stat tile (queue depths) renders a real, non-placeholder
  // number — not blank, not the null-safe "—" fallback used elsewhere.
  const queueCard = page.locator("section", {
    has: page.locator("h3", { hasText: "Queue depths" }),
  });
  const firstStatValue = queueCard.locator(".adm-num").first();
  await expect(firstStatValue).toBeVisible();
  const statText = (await firstStatValue.textContent())?.trim() ?? "";
  expect(statText.length).toBeGreaterThan(0);
  expect(statText).not.toBe("—");

  expect(pageErrors).toEqual([]);
});

test("GET /admin/coverage renders the coverage heatmap and a us_house regime row", async ({
  page,
}) => {
  const pageErrors: Error[] = [];
  page.on("pageerror", (error) => pageErrors.push(error));

  const response = await page.goto("/admin/coverage");
  expect(response?.status()).toBe(200);
  await expect(page.getByRole("heading", { name: "World coverage", level: 1 })).toBeVisible();

  // The signature element: the dense per-jurisdiction phase grid.
  await expect(
    page.getByRole("img", { name: /Coverage phase for \d+ jurisdictions/ }),
  ).toBeVisible();

  // us_house is a real, fixture-seeded, bridged adapter (crates/adapters/us_house)
  // — its regime code surfaces in the regime coverage table's "bridge" column.
  await expect(page.locator("body")).toContainText("us_house");

  expect(pageErrors).toEqual([]);
});

test("GET /admin/loop renders some content without crashing", async ({ page }) => {
  const pageErrors: Error[] = [];
  page.on("pageerror", (error) => pageErrors.push(error));

  // Whether GOVFOLIO_REPO_ROOT was set when the API server under test was
  // started decides which of two honest states renders: a 503 -> the shared
  // <Unavailable/> panel (cloud posture, no repo checkout mounted), or the
  // full goal-queue page. Either is a pass — only a crash is a failure.
  const response = await page.goto("/admin/loop");
  expect(response?.status()).toBe(200);

  const bodyText = await page.locator("body").innerText();
  const isUnavailable = bodyText.includes("Unavailable in this environment");
  const isGoalsList = bodyText.includes("Goal queue");
  expect(isUnavailable || isGoalsList).toBe(true);

  expect(pageErrors).toEqual([]);
});
