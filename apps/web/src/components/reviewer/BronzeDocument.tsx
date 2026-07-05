import type { Provenance } from "@/lib/api";
import { formatDateTime } from "@/lib/format";

// Right half of the side-by-side (design §7.2): the Bronze document. Today
// this embeds the OFFICIAL source PDF by its recorded URL; serving our
// archived GCS copy (signed URLs onto the bronze bucket) is post-020-apply —
// until then the sha256 below is the integrity anchor for what we archived.
export function BronzeDocument({ provenance }: { provenance: Provenance }) {
  const { raw_document } = provenance;
  return (
    <section className="bronze-doc" aria-label="Source document">
      <h2>Source document</h2>
      {raw_document.source_url ? (
        <>
          <iframe
            className="doc-frame"
            src={raw_document.source_url}
            title="Official source document"
          />
          <p className="doc-fallback">
            If the document does not render,{" "}
            <a href={raw_document.source_url} rel="noopener noreferrer">
              open the official PDF directly
            </a>
            .
          </p>
        </>
      ) : (
        <p className="muted">
          Source URL not recorded for this document — verify against the archived
          Bronze bytes by sha256.
        </p>
      )}
      <p className="doc-integrity">
        Archived copy fetched {formatDateTime(raw_document.fetched_at)}
        <span className="sha" data-testid="bronze-sha256">
          sha256:{raw_document.sha256}
        </span>
      </p>
    </section>
  );
}
