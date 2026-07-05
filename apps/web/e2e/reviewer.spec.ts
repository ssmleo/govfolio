// Reviewer console flows (goal 041) against the REAL seeded API: every
// verdict below goes through the resolve endpoint onto pipeline promote —
// the specs then verify what the single write authority actually did.
import { expect, test } from "@playwright/test";

import type { RecordDetail, ReviewAuditEntry, ReviewTaskDetail } from "./api";
import { API_URL, apiGet } from "./api";
import { seedReviewCase } from "./reviewer-db";

test("queue → task → confirm flow, with side-by-side and audit log", async ({
  page,
}) => {
  const { taskId, recordId } = await seedReviewCase({ priority: 9.7 });
  const detail = await apiGet<ReviewTaskDetail>(`/v1/review-tasks/${taskId}`);
  const record = detail.record;
  expect(record).toBeTruthy();
  if (!record) return;

  // QUEUE — reviewer surface is noindexed and shows the ranked open tasks.
  await page.goto("/review");
  await expect(page.locator('meta[name="robots"]')).toHaveAttribute(
    "content",
    /noindex/,
  );
  const taskLink = page.locator(`a[href="/review/${taskId}"]`);
  await expect(taskLink).toBeVisible();
  const row = page.locator("tr.queue-row", { has: taskLink });
  await expect(row).toContainText(record.record.asset_description_raw);
  await expect(row).toContainText(record.record.extracted_by);

  // TASK — side-by-side: extracted fields beside the Bronze document.
  await taskLink.click();
  await page.waitForURL(`**/review/${taskId}`);
  await expect(page.getByTestId("field-asset")).toHaveText(
    record.record.asset_description_raw,
  );
  await expect(page.getByTestId("bronze-sha256")).toHaveText(
    `sha256:${record.provenance.raw_document.sha256}`,
  );
  const sourceUrl = record.provenance.raw_document.source_url;
  if (sourceUrl) {
    await expect(page.locator("iframe.doc-frame")).toHaveAttribute("src", sourceUrl);
  }
  // Pre-review note: extraction context for the record.
  await expect(page.getByTestId("note-extractor")).toHaveText(
    record.record.extracted_by,
  );

  // CONFIRM — through the resolve endpoint only.
  await page.getByLabel("Reviewer").fill("e2e-confirm");
  await page.getByLabel("Note", { exact: true }).fill("matches the source document");
  await page.getByRole("button", { name: "Confirm" }).click();

  const outcome = page.getByTestId("resolve-outcome");
  await expect(outcome).toBeVisible();
  await expect(outcome).toContainText("Verdict applied.");
  await expect(outcome.locator(`a[href="/r/${recordId}"]`)).toBeVisible();

  // The reloaded server state shows the resolution + the audit row.
  await expect(page.locator(".task-meta [data-status]")).toHaveAttribute(
    "data-status",
    "resolved",
  );
  const auditRow = page.locator("tr.audit-row");
  await expect(auditRow).toHaveCount(1);
  await expect(auditRow).toContainText("e2e-confirm");
  await expect(auditRow).toContainText("confirm");
  await expect(auditRow).toContainText("applied");
  await expect(auditRow).toContainText("matches the source document");

  // What promote actually did: the record is now verified.
  const after = await apiGet<RecordDetail>(`/v1/records/${recordId}`);
  expect(after.record.verification_state).toBe("verified");
});

test("edit flow: correction supersedes through promote and the chain appears", async ({
  page,
}) => {
  const { taskId, recordId } = await seedReviewCase({ priority: 9.6 });
  const before = await apiGet<ReviewTaskDetail>(`/v1/review-tasks/${taskId}`);
  const original = before.record;
  expect(original).toBeTruthy();
  if (!original) return;

  await page.goto(`/review/${taskId}`);
  await page.getByLabel("Reviewer").fill("e2e-edit");
  await page.getByLabel("Note", { exact: true }).fill("band was one too low");
  await page.getByRole("button", { name: "Edit…" }).click();

  // The field-level form is seeded with the record's CURRENT values.
  await expect(page.getByLabel("Asset description (as filed)")).toHaveValue(
    original.record.asset_description_raw,
  );
  if (original.record.value) {
    await expect(page.getByLabel("Value low")).toHaveValue(original.record.value.low);
  }

  // Correct the declared band (decimal STRINGS end to end).
  await page.getByLabel("Value low").fill("15001.00");
  await page.getByLabel("Value high").fill("50000.00");
  await page.getByLabel("Regime code").fill("us_house");
  await page.getByRole("button", { name: "Submit correction" }).click();

  const outcome = page.getByTestId("resolve-outcome");
  await expect(outcome).toBeVisible();
  const supersedingLink = page.getByTestId("superseding-link");
  await expect(supersedingLink).toBeVisible();
  const supersedingHref = await supersedingLink.getAttribute("href");
  expect(supersedingHref).toBeTruthy();
  const supersedingId = supersedingHref?.replace("/r/", "") ?? "";

  // After the reload, the supersession chain appears on the task page.
  const chain = page.getByTestId("superseded-by");
  await expect(chain).toBeVisible();
  await expect(chain.locator(`a[href="/r/${supersedingId}"]`)).toBeVisible();

  // What promote actually did: a corrected row superseding the original,
  // original untouched (supersede, never update).
  const after = await apiGet<RecordDetail>(`/v1/records/${recordId}`);
  expect(after.record.verification_state).toBe("unverified");
  const corrected = after.superseded_by.find((r) => r.id === supersedingId);
  expect(corrected).toBeTruthy();
  expect(corrected?.verification_state).toBe("corrected");
  expect(corrected?.supersedes_record_id).toBe(recordId);
  expect(corrected?.value?.low).toBe("15001.00");
  expect(corrected?.value?.high).toBe("50000.00");
});

test("reject flow: the record becomes disputed and the verdict is audited", async ({
  page,
}) => {
  const { taskId, recordId } = await seedReviewCase({ priority: 9.5 });

  await page.goto(`/review/${taskId}`);
  await page.getByLabel("Reviewer").fill("e2e-reject");
  await page.getByLabel("Note", { exact: true }).fill("does not match the filing");
  await page.getByRole("button", { name: "Reject" }).click();

  await expect(page.getByTestId("resolve-outcome")).toContainText("Verdict applied.");
  await expect(page.locator(".task-meta [data-status]")).toHaveAttribute(
    "data-status",
    "resolved",
  );

  const after = await apiGet<RecordDetail>(`/v1/records/${recordId}`);
  expect(after.record.verification_state).toBe("disputed");

  const audit = await apiGet<ReviewAuditEntry[]>(`/v1/review-tasks/${taskId}/audit`);
  expect(audit).toHaveLength(1);
  expect(audit[0]?.verdict).toBe("reject");
  expect(audit[0]?.outcome).toBe("applied");
  expect(audit[0]?.reviewer).toBe("e2e-reject");
});

test("409 already-resolved is handled honestly: nothing changes, state reloads", async ({
  page,
}) => {
  const { taskId } = await seedReviewCase({ priority: 9.4 });

  await page.goto(`/review/${taskId}`);
  await page.getByLabel("Reviewer").fill("e2e-late");

  // Another reviewer wins the race — through the same resolve endpoint.
  const raced = await fetch(`${API_URL}/v1/review-tasks/${taskId}/resolve`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ reviewer: "e2e-race", verdict: "confirm" }),
  });
  expect(raced.status).toBe(200);

  await page.getByRole("button", { name: "Confirm" }).click();
  await expect(page.getByTestId("resolve-conflict")).toContainText("Already resolved");

  // The reloaded state shows what really happened: resolved, two audit rows
  // (the applied confirm, then this conflicting attempt).
  await expect(page.locator(".task-meta [data-status]")).toHaveAttribute(
    "data-status",
    "resolved",
  );
  await expect(page.locator("tr.audit-row")).toHaveCount(2);
  await expect(page.locator('tr.audit-row [data-outcome="conflict"]')).toBeVisible();
});
