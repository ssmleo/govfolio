import { expect, test } from "@playwright/test";

import { seededRecords } from "./api";

test("sitemap.xml is an index over politicians, records, jurisdictions", async ({
  request,
}) => {
  const res = await request.get("/sitemap.xml");
  expect(res.status()).toBe(200);
  expect(res.headers()["content-type"]).toContain("xml");
  const body = await res.text();
  expect(body).toContain("<sitemapindex");
  expect(body).toContain("/sitemaps/politicians.xml");
  expect(body).toContain("/sitemaps/records.xml");
  expect(body).toContain("/sitemaps/jurisdictions.xml");
});

test("child sitemaps enumerate real entity URLs from the API", async ({ request }) => {
  const records = await seededRecords();
  const first = records[0];
  expect(first).toBeTruthy();
  if (!first) return;

  const recordsMap = await request.get("/sitemaps/records.xml");
  expect(recordsMap.status()).toBe(200);
  expect(await recordsMap.text()).toContain(`/r/${first.id}</loc>`);

  const politiciansMap = await request.get("/sitemaps/politicians.xml");
  expect(politiciansMap.status()).toBe(200);
  expect(await politiciansMap.text()).toContain(`/p/${first.politician_id}</loc>`);

  const jurisdictionsMap = await request.get("/sitemaps/jurisdictions.xml");
  expect(jurisdictionsMap.status()).toBe(200);
  expect(await jurisdictionsMap.text()).toContain("/jurisdictions</loc>");
});

test("sitemaps carry CDN cache headers", async ({ request }) => {
  const res = await request.get("/sitemap.xml");
  const cacheControl = res.headers()["cache-control"] ?? "";
  expect(cacheControl).toMatch(/s-maxage=\d+/);
});
