import { expect, test } from "@playwright/test";

import { seededRecords } from "./api";

// Design §6.4: CDN-cached SSR is the read-scaling story. ISR pages must
// emit s-maxage + stale-while-revalidate so the CDN can serve and refresh.
test("ISR pages emit CDN cache headers (s-maxage + stale-while-revalidate)", async ({
  request,
}) => {
  for (const path of ["/", "/jurisdictions"]) {
    const res = await request.get(path);
    expect(res.status()).toBe(200);
    const cacheControl = res.headers()["cache-control"] ?? "";
    expect(cacheControl, `${path} cache-control`).toMatch(/s-maxage=\d+/);
    expect(cacheControl, `${path} cache-control`).toContain("stale-while-revalidate");
  }
});

test("record pages are CDN-cacheable too", async ({ request }) => {
  const records = await seededRecords();
  const first = records[0];
  if (!first) return;
  const res = await request.get(`/r/${first.id}`);
  expect(res.status()).toBe(200);
  expect(res.headers()["cache-control"] ?? "").toMatch(/s-maxage=\d+/);
});
