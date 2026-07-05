import { listRecords } from "@/lib/api";
import { urlsetXml, walkCursor, xmlResponse } from "@/lib/sitemap";

export const revalidate = 3600;

export async function GET(): Promise<Response> {
  const records = await walkCursor((cursor) =>
    listRecords(cursor === undefined ? { limit: 200 } : { limit: 200, cursor }),
  );
  return xmlResponse(urlsetXml(records.map((record) => `/r/${record.id}`)));
}
