import { listPoliticians } from "@/lib/api";
import { urlsetXml, walkCursor, xmlResponse } from "@/lib/sitemap";

export const revalidate = 3600;

export async function GET(): Promise<Response> {
  const politicians = await walkCursor((cursor) =>
    listPoliticians(cursor === undefined ? { limit: 200 } : { limit: 200, cursor }),
  );
  return xmlResponse(urlsetXml(politicians.map((politician) => `/p/${politician.id}`)));
}
