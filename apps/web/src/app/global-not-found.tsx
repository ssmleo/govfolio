// No imports in this file — vercel/next.js#92256 (infinite dev loop). Inline everything.
//
// Serves genuinely unmatched paths (e.g. a typo'd URL) that don't fall under
// any route group at all. With (site) and (admin) as two separate root
// layouts, there's no single shared layout left to compose a fallback 404
// from, so this file owns its own <html><body> per Next's global-not-found
// convention (see next.config.ts `experimental.globalNotFound`). Paths that
// resolve under (site) still get its normal not-found.tsx, nested in (site)'s
// layout/header/footer as before.
//
// Styling and copy below are hand-duplicated (not imported from
// (site)/globals.css or any component) so this file has zero import
// statements — any import here reproduces the infinite dev-mode loop in
// Next 16.2.x described in the issue above.

export const metadata = {
  title: "Not found · govfolio",
  description:
    "Nothing is published at this address. It may have been superseded or never existed.",
};

export default function GlobalNotFound() {
  return (
    <html lang="en">
      <body>
        <style>{`
          :root { color-scheme: dark; }
          * { box-sizing: border-box; }
          body {
            margin: 0;
            min-height: 100vh;
            display: flex;
            align-items: center;
            justify-content: center;
            background: #0e1512;
            color: #e7ede9;
            font-family: -apple-system, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
            line-height: 1.55;
          }
          .not-found-shell {
            max-width: 32rem;
            margin: 0 auto;
            padding: 2rem 1.25rem;
            text-align: center;
          }
          .not-found-shell h1 {
            font-size: clamp(1.6rem, 4vw, 2.2rem);
            margin: 0 0 0.75rem;
            color: #f5f7f6;
          }
          .not-found-shell p {
            color: #a7b3ad;
            margin: 0 0 1.5rem;
          }
          .not-found-shell a {
            color: #4fd8a4;
            text-decoration-thickness: 1px;
            text-underline-offset: 2px;
          }
          .not-found-shell a:hover {
            color: #7de6bf;
          }
        `}</style>
        <main className="not-found-shell">
          <h1>Not found</h1>
          <p>
            Nothing is published at this address. It may have been
            superseded or never existed.
          </p>
          <p>
            <a href="/">Back to the latest records</a>
          </p>
        </main>
      </body>
    </html>
  );
}
