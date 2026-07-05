// E2E helper: reads the seeded API directly (typed by the generated
// contract) so specs assert against real data instead of hardcoded ids.
import type { components } from "@govfolio/contracts";

type Schemas = components["schemas"];
export type DisclosureRecord = Schemas["DisclosureRecord"];
export type PoliticianProfile = Schemas["PoliticianProfile"];
export type RecordDetail = Schemas["RecordDetail"];
export type RecordPage = Schemas["RecordPage"];
export type ReviewTaskDetail = Schemas["ReviewTaskDetail"];
export type ReviewAuditEntry = Schemas["ReviewAuditEntry"];

export const API_URL = process.env.GOVFOLIO_API_URL ?? "http://localhost:8080";

// The review surface is X-Admin-Token-gated (goal 050). OBVIOUS DUMMY test
// value — the locally running API must be started with the SAME token:
//   ADMIN_TOKEN=govfolio-e2e-admin-dummy cargo run -p api
// The playwright webServer forwards it to the web process as
// GOVFOLIO_ADMIN_TOKEN (see playwright.config.ts).
export const ADMIN_TOKEN = process.env.GOVFOLIO_ADMIN_TOKEN ?? "govfolio-e2e-admin-dummy";

export async function apiGet<T>(path: string, options?: { admin?: boolean }): Promise<T> {
  const headers: Record<string, string> = options?.admin
    ? { "x-admin-token": ADMIN_TOKEN }
    : {};
  let res: Response;
  try {
    res = await fetch(`${API_URL}${path}`, { headers });
  } catch (error) {
    throw new Error(
      `govfolio API unreachable at ${API_URL}. Start it first:\n` +
        `  scripts/dev/pg-local.ps1 start\n` +
        `  cargo run -p worker --bin local   (seed)\n` +
        `  ADMIN_TOKEN=govfolio-e2e-admin-dummy cargo run -p api   (:8080)`,
      { cause: error },
    );
  }
  if (!res.ok) {
    throw new Error(`GET ${path} responded ${res.status}`);
  }
  return (await res.json()) as T;
}

export async function seededRecords(): Promise<DisclosureRecord[]> {
  const page = await apiGet<RecordPage>("/v1/records?limit=200");
  if (page.items.length === 0) {
    throw new Error("API has no records — seed with: cargo run -p worker --bin local");
  }
  return page.items;
}
