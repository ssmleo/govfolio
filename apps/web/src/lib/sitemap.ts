// Sitemap building (design §6.4: SEO is the free tier's growth engine —
// permanent, CDN-cached URLs for every entity; sitemaps enumerate them).
//
// /sitemap.xml is an index of per-entity sitemaps so each section can grow
// (and later shard) independently; a single urlset caps at 50k URLs.

const SITEMAP_URL_CAP = 50_000;

export function siteBaseUrl(): string {
  return process.env.GOVFOLIO_SITE_URL ?? "https://govfolio.io";
}

function xmlEscape(text: string): string {
  return text
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll("'", "&apos;")
    .replaceAll('"', "&quot;");
}

export function urlsetXml(paths: string[]): string {
  const base = siteBaseUrl();
  const urls = paths
    .slice(0, SITEMAP_URL_CAP)
    .map((path) => `  <url><loc>${xmlEscape(`${base}${path}`)}</loc></url>`)
    .join("\n");
  return `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${urls}\n</urlset>\n`;
}

export function sitemapIndexXml(paths: string[]): string {
  const base = siteBaseUrl();
  const sitemaps = paths
    .map((path) => `  <sitemap><loc>${xmlEscape(`${base}${path}`)}</loc></sitemap>`)
    .join("\n");
  return `<?xml version="1.0" encoding="UTF-8"?>\n<sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n${sitemaps}\n</sitemapindex>\n`;
}

export function xmlResponse(xml: string): Response {
  return new Response(xml, {
    headers: {
      "content-type": "application/xml; charset=utf-8",
      "cache-control": "public, s-maxage=3600, stale-while-revalidate=86400",
    },
  });
}

/** Walks a cursor-paginated listing to exhaustion (bounded by the URL cap). */
export async function walkCursor<T>(
  fetchPage: (cursor: string | undefined) => Promise<{
    items: T[];
    next_cursor?: string | null;
  }>,
): Promise<T[]> {
  const all: T[] = [];
  let cursor: string | undefined = undefined;
  for (;;) {
    const page = await fetchPage(cursor);
    all.push(...page.items);
    if (
      page.next_cursor === null ||
      page.next_cursor === undefined ||
      all.length >= SITEMAP_URL_CAP
    ) {
      return all;
    }
    cursor = page.next_cursor;
  }
}
