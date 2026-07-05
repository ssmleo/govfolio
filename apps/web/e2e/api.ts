// E2E helper: reads the seeded API directly (typed by the generated
// contract) so specs assert against real data instead of hardcoded ids.
import type { components } from "@govfolio/contracts";

type Schemas = components["schemas"];
export type DisclosureRecord = Schemas["DisclosureRecord"];
export type PoliticianProfile = Schemas["PoliticianProfile"];
export type RecordDetail = Schemas["RecordDetail"];
export type RecordPage = Schemas["RecordPage"];

export const API_URL = process.env.GOVFOLIO_API_URL ?? "http://localhost:8080";

export async function apiGet<T>(path: string): Promise<T> {
  let res: Response;
  try {
    res = await fetch(`${API_URL}${path}`);
  } catch (error) {
    throw new Error(
      `govfolio API unreachable at ${API_URL}. Start it first:\n` +
        `  scripts/dev/pg-local.ps1 start\n` +
        `  cargo run -p worker --bin local   (seed)\n` +
        `  cargo run -p api                  (:8080)`,
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
