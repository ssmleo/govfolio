import type { Metadata } from "next";
import Link from "next/link";

import "./globals.css";

export const metadata: Metadata = {
  title: {
    default: "govfolio — politician financial disclosures",
    template: "%s · govfolio",
  },
  description:
    "Politician financial-disclosure records, each traced to its official source document.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>
        <header className="site-header">
          <div className="shell site-header-inner">
            <Link href="/" className="wordmark">
              govfolio
            </Link>
            <nav aria-label="Primary">
              <Link href="/jurisdictions">Jurisdictions</Link>
              <Link href="/corrections">Corrections</Link>
            </nav>
            <form action="/search" method="get" role="search" className="header-search">
              <label className="visually-hidden" htmlFor="header-q">
                Search politicians and instruments
              </label>
              <input
                id="header-q"
                type="search"
                name="q"
                placeholder="Search politicians, instruments"
                required
              />
              <button type="submit">Search</button>
            </form>
          </div>
        </header>
        <main className="shell">{children}</main>
        <footer className="site-footer">
          <div className="shell">
            <p>
              govfolio · disclosure records as filed with official sources ·{" "}
              <Link href="/jurisdictions">coverage by jurisdiction</Link> ·{" "}
              <Link href="/corrections">corrections</Link>
            </p>
          </div>
        </footer>
      </body>
    </html>
  );
}
