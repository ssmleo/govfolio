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

// The instrument-panel clock (AdminClock, mounted once in Masthead) renders
// a "--:--:-- UTC" placeholder on the server and only starts ticking a real
// HH:MM:SS after React hydrates and its useEffect fires. Waiting on it is a
// cheap, non-flaky "hydration is done" signal for the whole admin tree below
// it (one shared (admin) root layout) — needed before dispatching keyboard
// shortcuts or clicking rows whose handlers are wired client-side.
async function waitForAdminHydration(page: import("@playwright/test").Page): Promise<void> {
  await expect(page.getByText(/^\d{2}:\d{2}:\d{2} UTC$/)).toBeVisible();
}

test("GET /admin renders the instrument-panel shell: masthead, grouped sidebar, sentinel ticker", async ({
  page,
}) => {
  const pageErrors: Error[] = [];
  page.on("pageerror", (error) => pageErrors.push(error));

  await page.goto("/admin");

  // Masthead: wordmark, console tag, and (once hydrated) a live clock.
  await expect(page.getByRole("link", { name: "govfolio" })).toBeVisible();
  await expect(page.getByText("Administrative Console")).toBeVisible();
  await waitForAdminHydration(page);

  // Sidebar: all 5 pipeline-phase group labels from the grouped nav (goal 094).
  const sidebar = page.getByRole("navigation", { name: "Admin sections" });
  for (const label of ["Command", "Acquisition", "Refinery", "Platform", "Autonomy"]) {
    await expect(sidebar.getByText(label, { exact: true })).toBeVisible();
  }

  // Sentinel ticker: assert on labels unique to the ticker itself. "review
  // open" / "drift open" / "running" also appear in Overview's own "Queue
  // depths" / "Pipeline runs" cards, so they'd be ambiguous here — "failed
  // 24h" and "dlq" are ticker-only labels on this page.
  await expect(page.getByText("failed 24h", { exact: true })).toBeVisible();
  await expect(page.getByText("dlq", { exact: true })).toBeVisible();

  expect(pageErrors).toEqual([]);
});

test("digit shortcut '2' on /admin navigates to /admin/coverage", async ({ page }) => {
  await page.goto("/admin");
  await waitForAdminHydration(page);

  await page.keyboard.press("2");
  await expect(page).toHaveURL(/\/admin\/coverage$/);
});

test("clicking a seeded us_house row opens the regime dossier with adapter/bridge facts; Escape closes it", async ({
  page,
}) => {
  await page.goto("/admin/coverage");
  await waitForAdminHydration(page);

  // The regime-coverage table row-click contract (Table.tsx): a row with a
  // click handler is exposed as an accessible "button" whose name is its
  // full cell text — including the "bridge" column's regime code(s).
  const usHouseRow = page.getByRole("button", { name: /us_house/ });
  await expect(usHouseRow).toBeVisible();
  await usHouseRow.click();

  const dossier = page.getByRole("complementary", { name: /Regime dossier/ });
  await expect(dossier).toBeVisible();
  await expect(dossier).toContainText("bridge code(s)");
  await expect(dossier).toContainText("us_house");
  await expect(dossier).toContainText("crates/adapters/us_house");

  await page.keyboard.press("Escape");
  await expect(dossier).not.toBeVisible();
});
