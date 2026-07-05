import type { DisclosureRecord } from "@/lib/api";
import { formatConfidence, formatDate, formatValueInterval } from "@/lib/format";
import { VerificationBadge } from "@/components/VerificationBadge";

// Left half of the side-by-side (design §7.2): EVERY extracted field of the
// target record, so the reviewer compares our extraction against the Bronze
// document field by field.
export function TaskFields({ record }: { record: DisclosureRecord }) {
  return (
    <section className="task-fields" aria-label="Extracted fields">
      <h2>Extracted fields</h2>
      <dl className="facts">
        <dt>Asset as filed</dt>
        <dd data-testid="field-asset">{record.asset_description_raw}</dd>

        <dt>Type</dt>
        <dd>
          {record.record_type.replaceAll("_", " ")}
          {record.side ? ` · ${record.side}` : null}
        </dd>

        <dt>Asset class</dt>
        <dd>{record.asset_class.replaceAll("_", " ")}</dd>

        <dt>Declared value</dt>
        <dd className="cell-value" data-testid="field-value">
          {record.value ? formatValueInterval(record.value) : "Not declared"}
        </dd>

        <dt>Transaction date</dt>
        <dd>{record.transaction_date ? formatDate(record.transaction_date) : "—"}</dd>

        <dt>Notified date</dt>
        <dd>{record.notified_date ? formatDate(record.notified_date) : "—"}</dd>

        <dt>As-of date</dt>
        <dd>{record.as_of_date ? formatDate(record.as_of_date) : "—"}</dd>

        <dt>Owner</dt>
        <dd data-testid="field-owner">{record.owner ?? "unknown"}</dd>

        <dt>Instrument</dt>
        <dd>
          {record.instrument_id ? (
            <span className="mono">{record.instrument_id}</span>
          ) : (
            <span className="muted">Not resolved — left unlinked rather than guessed</span>
          )}
        </dd>

        <dt>State</dt>
        <dd>
          <VerificationBadge state={record.verification_state} />
        </dd>

        <dt>Extracted by</dt>
        <dd className="mono">{record.extracted_by}</dd>

        <dt>Confidence</dt>
        <dd>
          {record.extraction_confidence != null
            ? formatConfidence(record.extraction_confidence)
            : "—"}
        </dd>

        <dt>Fingerprint</dt>
        <dd className="mono">{record.fingerprint}</dd>

        <dt>Record ID</dt>
        <dd className="mono">{record.id}</dd>
      </dl>

      <details className="payload" open>
        <summary>Regime payload (as extracted)</summary>
        <pre data-testid="field-details">{JSON.stringify(record.details, null, 2)}</pre>
      </details>
    </section>
  );
}
