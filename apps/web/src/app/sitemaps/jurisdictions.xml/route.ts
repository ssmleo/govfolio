import { listJurisdictions } from "@/lib/api";
import { urlsetXml, xmlResponse } from "@/lib/sitemap";

export const revalidate = 3600;

export async function GET(): Promise<Response> {
  const jurisdictions = await listJurisdictions();
  return xmlResponse(
    urlsetXml([
      "/",
      "/jurisdictions",
      ...jurisdictions.map(
        (jurisdiction) => `/jurisdictions/${encodeURIComponent(jurisdiction.id)}`,
      ),
    ]),
  );
}
