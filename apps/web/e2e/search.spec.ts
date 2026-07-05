import { expect, test } from "@playwright/test";

import type { PoliticianProfile } from "./api";
import { apiGet, seededRecords } from "./api";

test("search finds a politician by name substring and links to the profile", async ({
  page,
}) => {
  const records = await seededRecords();
  const politicianId = records[0]?.politician_id;
  const profile = await apiGet<PoliticianProfile>(`/v1/politicians/${politicianId}`);
  const lastName = profile.canonical_name.split(" ").at(-1) ?? profile.canonical_name;

  await page.goto(`/search?q=${encodeURIComponent(lastName)}`);
  const hit = page.getByRole("link", { name: profile.canonical_name });
  await expect(hit).toBeVisible();
  await hit.click();
  await expect(page).toHaveURL(new RegExp(`/p/${profile.id}$`));
});

test("search page renders both arms with honest empty states", async ({ page }) => {
  await page.goto("/search?q=zzzzzz-no-such-entity");
  await expect(page.getByText(/No politicians matched/)).toBeVisible();
  await expect(page.getByText(/No instruments matched/)).toBeVisible();
});

test("home search box submits to /search", async ({ page }) => {
  await page.goto("/");
  await page.locator("#home-q").fill("pelosi");
  await page.locator(".hero-search button").click();
  await expect(page).toHaveURL(/\/search\?q=pelosi/);
});
