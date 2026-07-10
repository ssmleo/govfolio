import type { Metadata } from "next";
import Link from "next/link";

import type { SearchResults } from "@/lib/api";
import { ApiError, search } from "@/lib/api";

// Search reads the query string, so it renders dynamically — correct for a
// CDN: results must not be cached across queries.

export const metadata: Metadata = {
  title: "Search",
  description: "Search politicians and instruments across disclosure records.",
};

interface Props {
  searchParams: Promise<{ q?: string | string[] }>;
}

async function runSearch(q: string): Promise<SearchResults | null> {
  try {
    return await search(q);
  } catch (error) {
    if (error instanceof ApiError && error.status === 400) {
      return null; // blank query — render the empty prompt instead
    }
    throw error;
  }
}

export default async function SearchPage({ searchParams }: Props) {
  const params = await searchParams;
  const raw = Array.isArray(params.q) ? params.q[0] : params.q;
  const q = raw?.trim() ?? "";
  const results = q.length > 0 ? await runSearch(q) : null;

  return (
    <>
      <section className="profile-head">
        <h1>Search</h1>
        <form action="/search" method="get" role="search" className="hero-search">
          <label className="visually-hidden" htmlFor="search-q">
            Search politicians and instruments
          </label>
          <input
            id="search-q"
            type="search"
            name="q"
            defaultValue={q}
            placeholder="Politician name, instrument name, or ticker"
            required
          />
          <button type="submit">Search</button>
        </form>
      </section>

      {results === null ? (
        <p className="muted">
          Type a politician’s name, an instrument name, or a ticker to search.
        </p>
      ) : (
        <div className="search-arms">
          <section aria-label="Politicians">
            <h2>Politicians</h2>
            {results.politicians.length === 0 ? (
              <p className="empty">No politicians matched “{results.query}”.</p>
            ) : (
              <ul className="hits">
                {results.politicians.map((politician) => (
                  <li key={politician.id}>
                    <Link href={`/p/${politician.id}`}>{politician.canonical_name}</Link>
                  </li>
                ))}
              </ul>
            )}
          </section>
          <section aria-label="Instruments">
            <h2>Instruments</h2>
            {results.instruments.length === 0 ? (
              <p className="empty">No instruments matched “{results.query}”.</p>
            ) : (
              <ul className="hits">
                {results.instruments.map((instrument) => (
                  <li key={instrument.id}>
                    {instrument.name}
                    {instrument.ticker ? (
                      <span className="mono"> ({instrument.ticker})</span>
                    ) : null}
                    <span className="muted"> · {instrument.asset_class}</span>
                  </li>
                ))}
              </ul>
            )}
          </section>
        </div>
      )}
    </>
  );
}
