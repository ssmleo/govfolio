import type { Metadata } from "next";
import Link from "next/link";

// Serves genuinely unmatched paths (e.g. a typo'd URL) that don't fall under
// any route group at all. With (site) and (admin) as two separate root
// layouts, there's no single shared layout left to compose a fallback 404
// from, so this file owns its own <html><body> per Next's global-not-found
// convention (see next.config.ts `experimental.globalNotFound`). Paths that
// resolve under (site) still get its normal not-found.tsx, nested in (site)'s
// layout/header/footer as before.
import "./(site)/globals.css";

export const metadata: Metadata = {
  title: "Not found · govfolio",
  description:
    "Nothing is published at this address. It may have been superseded or never existed.",
};

export default function GlobalNotFound() {
  return (
    <html lang="en">
      <body>
        <main className="shell">
          <section className="profile-head">
            <h1>Not found</h1>
            <p className="muted">
              Nothing is published at this address. It may have been
              superseded or never existed.
            </p>
            <p>
              <Link href="/">Back to the latest records</Link> ·{" "}
              <Link href="/search">Search</Link>
            </p>
          </section>
        </main>
      </body>
    </html>
  );
}
