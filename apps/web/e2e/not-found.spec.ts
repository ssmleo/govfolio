// Regression check for goal 094's route-group split ((site)/(admin) as two
// separate Next.js root layouts, next.config.ts `experimental.globalNotFound`).
// Two genuinely different 404 shapes now exist and must stay distinct:
//   1. A path that resolves under (site) but whose entity doesn't exist
//      (e.g. an unknown politician id) -> (site)'s own not-found.tsx, nested
//      in the normal site layout (header/footer intact).
//   2. A path that matches no route at all, in no route group -> served by
//      the standalone app/global-not-found.tsx (its own <html><body>, no
//      site chrome) instead of Next's bare built-in fallback.
import { expect, test } from "@playwright/test";

test("an in-context 404 within (site) keeps the branded header/footer chrome", async ({
  page,
}) => {
  const response = await page.goto("/p/does-not-exist-xyz");
  expect(response?.status()).toBe(404);

  await expect(page.locator("header.site-header")).toBeVisible();
  await expect(page.getByRole("link", { name: "govfolio" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Not found", level: 1 })).toBeVisible();
  await expect(page.locator("body")).toContainText(
    "Nothing is published at this address",
  );
  await expect(page.locator("footer.site-footer")).toBeVisible();
});

test("a genuinely unmatched path is served by the branded global-not-found, not Next's bare default", async ({
  page,
}) => {
  const response = await page.goto("/zzz-nowhere-xyz");
  expect(response?.status()).toBe(404);

  // No (site) route group applies here at all, so none of its chrome should
  // be present -- this is the concrete signal that global-not-found.tsx (not
  // some fallback nested under (site)'s layout) served the response.
  await expect(page.locator("header.site-header")).toHaveCount(0);
  await expect(page.locator("footer.site-footer")).toHaveCount(0);

  await expect(page.getByRole("heading", { name: "Not found", level: 1 })).toBeVisible();
  await expect(page.locator("body")).toContainText(
    "Nothing is published at this address",
  );
  // Not Next's own built-in default 404 copy.
  await expect(page.locator("body")).not.toContainText("This page could not be found");
});
