import { listJurisdictions } from "@/lib/api";
import { urlsetXml, xmlResponse } from "@/lib/sitemap";

export const revalidate = 3600;

export async function GET(): Promise<Response> {
  const jurisdictions = await listJurisdictions();
  return xmlResponse(
    urlsetXml([
      // Static top-level public pages ride along here (design §6.4: every
      // public URL belongs in a sitemap).
      "/",
      "/jurisdictions",
      "/corrections",
      ...jurisdictions.map(
        (jurisdiction) => `/jurisdictions/${encodeURIComponent(jurisdiction.id)}`,
      ),
    ]),
  );
}
