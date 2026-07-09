import type { Provenance } from "@/lib/api";
import { apiBaseUrl } from "@/lib/api";
import { formatDateTime } from "@/lib/format";

// Right half of the side-by-side (design §7.2): the Bronze document. Embeds
// OUR archived copy (design §7.3's original intent — the gov URL can rot,
// change, or, for Brazil, point at a nationwide bulk file instead of
// anything politician-specific).
export function BronzeDocument({ provenance }: { provenance: Provenance }) {
  const { raw_document, filing } = provenance;
  const archivedCopyUrl = `${apiBaseUrl()}/v1/filings/${encodeURIComponent(filing.id)}/document`;
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
