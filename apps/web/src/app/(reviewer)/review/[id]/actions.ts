"use server";

import type { ResolveActionResult, ResolveInput } from "@/lib/api";
import { ApiError, resolveReviewTask } from "@/lib/api";

/**
 * The one door reviewer verdicts go through: the API's resolve endpoint onto
 * pipeline promote (design §7.2). Outcomes are projected honestly:
 * - applied  → what promote did (record + optional superseding correction);
 * - conflict → 409, someone resolved first; nothing changed on this attempt;
 * - error    → the API's error envelope, verbatim (fail closed, no retries).
 */
export async function resolveTaskAction(
  taskId: string,
  input: ResolveInput,
): Promise<ResolveActionResult> {
  try {
    const response = await resolveReviewTask(taskId, input);
    return {
      kind: "applied",
      recordId: response.record_id,
      supersedingRecordId: response.superseding_record_id ?? null,
    };
  } catch (error) {
    if (error instanceof ApiError) {
      if (error.status === 409) {
        return { kind: "conflict", message: error.message };
      }
      return { kind: "error", code: error.code, message: error.message };
    }
    throw error;
  }
}
