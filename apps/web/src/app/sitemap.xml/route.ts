import { sitemapIndexXml, xmlResponse } from "@/lib/sitemap";

export const revalidate = 3600;

export function GET(): Response {
  return xmlResponse(
    sitemapIndexXml([
      "/sitemaps/politicians.xml",
      "/sitemaps/records.xml",
      "/sitemaps/jurisdictions.xml",
    ]),
  );
}
