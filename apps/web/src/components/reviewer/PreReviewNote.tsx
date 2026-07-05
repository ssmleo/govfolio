import type { ExtractionContext } from "@/lib/api";
import { formatConfidence, formatDateTime } from "@/lib/format";

// The pre-review note (design §7.2): what produced the extraction the
// reviewer is adjudicating — extractor tag, confidence, and the extraction-
// cache evidence (model + cross-check provenance) when the LLM seam made it.
export function PreReviewNote({ extraction }: { extraction: ExtractionContext | null }) {
  return (
    <section className="pre-review" aria-label="Pre-review note">
      <h2>Pre-review note</h2>
      {extraction ? (
        <dl className="facts">
          <dt>Extracted by</dt>
          <dd className="mono" data-testid="note-extractor">
            {extraction.extracted_by}
          </dd>

          <dt>Confidence</dt>
          <dd data-testid="note-confidence">
            {extraction.extraction_confidence != null
              ? formatConfidence(extraction.extraction_confidence)
              : "not recorded"}
          </dd>

          {extraction.cache ? (
            <>
              <dt>Extraction model</dt>
              <dd className="mono">
                {extraction.cache.model_id}
                <span className="muted">
                  {" "}
                  · cached {formatDateTime(extraction.cache.cached_at)}
                </span>
              </dd>

              <dt>Cross-check</dt>
              <dd>
                <pre className="cache-provenance" data-testid="note-cache-provenance">
                  {JSON.stringify(extraction.cache.provenance, null, 2)}
                </pre>
              </dd>
            </>
          ) : (
            <>
              <dt>Extraction model</dt>
              <dd className="muted">
                No extraction-cache evidence — deterministic (non-LLM) parse.
              </dd>
            </>
          )}
        </dl>
      ) : (
        <p className="muted">
          No extraction context — this task does not target a disclosure record.
        </p>
      )}
    </section>
  );
}
