import Link from "next/link";

import type { DisclosureRecord } from "@/lib/api";
import { listRecords } from "@/lib/api";
import { RecordTable } from "@/components/RecordRows";

// ISR: CDN gets `s-maxage`/`stale-while-revalidate`; the free tier is the
// delayed read path by design (§6.2), so short staleness is correct here.
export const revalidate = 300;

// The API pages records in ascending ULID order (§6.1); the newest live on
// the LAST page, so walk to it (bounded). Cheap while the corpus is small —
// each revalidation is mostly 304s via the ETag layer. When the corpus
// outgrows the guard, the honest fix is a `order=desc` param on /v1/records.
const PAGE_WALK_GUARD = 25;

async function latestRecords(count: number): Promise<DisclosureRecord[]> {
  let page = await listRecords({ limit: 200 });
  let walked = 0;
  while (page.next_cursor !== null && page.next_cursor !== undefined) {
    if (walked >= PAGE_WALK_GUARD) {
      break;
    }
    page = await listRecords({ limit: 200, cursor: page.next_cursor });
    walked += 1;
  }
  return page.items.slice(-count).reverse();
}

export default async function HomePage() {
  const records = await latestRecords(10);
  return (
    <>
      <section className="hero">
        <h1>Politician financial disclosures, traced to the source</h1>
        <p>
          Search disclosure records by politician or instrument. Every record
          links to the official filing it was extracted from.
        </p>
        <form action="/search" method="get" role="search" className="hero-search">
          <label className="visually-hidden" htmlFor="home-q">
            Search politicians and instruments
          </label>
          <input
            id="home-q"
            type="search"
            name="q"
            placeholder="Try a politician’s name or a ticker"
            required
          />
          <button type="submit">Search</button>
        </form>
      </section>

      <section aria-label="Latest records">
        <h2>Latest records</h2>
        <div className="table-scroll">
          <RecordTable records={records} caption="Latest disclosure records" />
        </div>
      </section>

      <section aria-label="Coverage">
        <h2>Coverage</h2>
        <p>
          What each jurisdiction discloses, how precisely, and on what
          schedule: <Link href="/jurisdictions">the jurisdiction scorecard</Link>.
        </p>
      </section>
    </>
  );
}
