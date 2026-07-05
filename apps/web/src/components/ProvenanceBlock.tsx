import Link from "next/link";

import type { Provenance } from "@/lib/api";
import { formatDate, formatDateTime } from "@/lib/format";

// The trust surface (design §7.3): official-source link, our archived copy
// (sha256 + fetched_at), the filing, and the regime it was filed under.
export function ProvenanceBlock({ provenance }: { provenance: Provenance }) {
  const { filing, raw_document, regime } = provenance;
  return (
    <section className="provenance" aria-label="Provenance">
      <h2>Provenance</h2>
      <dl className="provenance-grid">
        <dt>Official source</dt>
        <dd>
          {raw_document.source_url ? (
            <a href={raw_document.source_url} rel="noopener noreferrer">
              {raw_document.source_url}
            </a>
          ) : (
            <span className="muted">Source URL not recorded for this document</span>
          )}
        </dd>

        <dt>Archived copy</dt>
        <dd>
          fetched {formatDateTime(raw_document.fetched_at)}
          <span className="sha" data-testid="sha256">
            sha256:{raw_document.sha256}
          </span>
        </dd>

        <dt>Filing</dt>
        <dd>
          {filing.external_id ? `#${filing.external_id}` : filing.id}
          {filing.filed_date ? ` · filed ${formatDate(filing.filed_date)}` : null}
          {filing.published_at
            ? ` · published ${formatDateTime(filing.published_at)}`
            : null}
        </dd>

        <dt>Regime</dt>
        <dd>
          <Link href={`/jurisdictions/${encodeURIComponent(regime.jurisdiction_id)}`}>
            {regime.body}
          </Link>
          {" · "}
          {regime.regime_type.replaceAll("_", " ")}
          {regime.source_url ? (
            <>
              {" · "}
              <a href={regime.source_url} rel="noopener noreferrer">
                official disclosure site
              </a>
            </>
          ) : null}
        </dd>
      </dl>
    </section>
  );
}
