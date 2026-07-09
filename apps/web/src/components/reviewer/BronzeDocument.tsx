import type { Provenance } from "@/lib/api";
import { formatDateTime } from "@/lib/format";

// Right half of the side-by-side (design §7.2): the Bronze document. Embeds
// OUR archived copy (design §7.3's original intent — the gov URL can rot,
// change, or, for Brazil, point at a nationwide bulk file instead of
// anything politician-specific).
//
// This goes through the SAME-ORIGIN reviewer document proxy
// (/review/document/{filingId}), NOT the public /v1/filings/{id}/document
// endpoint directly: the public endpoint is tier-gated behind the 24h
// free-tier embargo, and a plain browser request has no way to authenticate
// as a paid tier — a fresh filing (exactly what the review queue exists to
// adjudicate) would silently 404. The proxy fetches the ADMIN-gated,
// real-time variant server-side (see review/document/[filingId]/route.ts).
export function BronzeDocument({ provenance }: { provenance: Provenance }) {
  const { raw_document, filing } = provenance;
  const archivedCopyUrl = `/review/document/${encodeURIComponent(filing.id)}`;
  return (
    <section className="bronze-doc" aria-label="Source document">
      <h2>Source document</h2>
      <iframe
        className="doc-frame"
        src={archivedCopyUrl}
        title="Archived source document"
      />
      <p className="doc-fallback">
        If the document does not render,{" "}
        <a href={archivedCopyUrl} rel="noopener noreferrer">
          open the archived document directly
        </a>
        .
      </p>
      <p className="doc-integrity">
        Archived copy fetched {formatDateTime(raw_document.fetched_at)}
        <span className="sha" data-testid="bronze-sha256">
          sha256:{raw_document.sha256}
        </span>
      </p>
    </section>
  );
}
