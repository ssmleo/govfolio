import { expect, test } from "@playwright/test";

import type { RecordDetail } from "./api";
import { apiGet, seededRecords } from "./api";

test("record page shows the full trust surface (design §7.3)", async ({ page }) => {
  const records = await seededRecords();
  const withValue = records.find((record) => record.value != null) ?? records[0];
  expect(withValue).toBeTruthy();
  if (!withValue) return;
  const detail = await apiGet<RecordDetail>(`/v1/records/${withValue.id}`);

  await page.goto(`/r/${detail.record.id}`);

  // The record itself, as filed
  await expect(page.getByRole("heading", { level: 1 })).toHaveText(
    detail.record.asset_description_raw,
  );

  // Verification badge — visually distinct, honest
  const badge = page.locator(".record-head .badge");
  await expect(badge).toBeVisible();
  await expect(badge).toHaveAttribute("data-state", detail.record.verification_state);

  // Confidence surfaces when < 1
  const confidence = detail.record.extraction_confidence;
  if (confidence != null && confidence < 1) {
    await expect(page.getByTestId("confidence")).toContainText(
      `${Math.round(confidence * 100)}%`,
    );
  }

  // Provenance: archived-copy sha256 + fetch time + official source
  await expect(page.getByTestId("sha256")).toHaveText(
    `sha256:${detail.provenance.raw_document.sha256}`,
  );
  const sourceUrl = detail.provenance.raw_document.source_url;
  if (sourceUrl) {
    await expect(
      page.locator(".provenance").getByRole("link", { name: sourceUrl }),
    ).toHaveAttribute("href", sourceUrl);
  } else {
    await expect(
      page.getByText("Source URL not recorded for this document"),
    ).toBeVisible();
  }

  // Supersession history: both directions, honestly empty when no corrections
  if (detail.supersedes.length === 0 && detail.superseded_by.length === 0) {
    await expect(page.getByTestId("no-supersession")).toBeVisible();
  } else {
    if (detail.superseded_by.length > 0) {
      await expect(page.getByTestId("superseded-by")).toBeVisible();
    }
    if (detail.supersedes.length > 0) {
      await expect(page.getByTestId("supersedes")).toBeVisible();
    }
  }
});

test("record page's archived-copy link actually serves the document", async ({
  page,
  request,
}) => {
  const records = await seededRecords();
  const withValue = records.find((record) => record.value != null) ?? records[0];
  expect(withValue).toBeTruthy();
  if (!withValue) return;
  const detail = await apiGet<RecordDetail>(`/v1/records/${withValue.id}`);

  await page.goto(`/r/${detail.record.id}`);

  const archivedLink = page.getByRole("link", { name: "Open archived copy" });
  const href = await archivedLink.getAttribute("href");
  expect(href).toBeTruthy();
  if (!href) return;

  const response = await request.get(href);
  expect(response.ok()).toBeTruthy();
  expect(response.headers()["content-type"]).toBeTruthy();
});
