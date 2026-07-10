import type { Metadata } from "next";
import Link from "next/link";
import { notFound } from "next/navigation";

import type { RecordDetail } from "@/lib/api";
import { ApiError, getRecord, politicianProfile } from "@/lib/api";
import { formatConfidence, formatDate, formatValueInterval } from "@/lib/format";
import { ProvenanceBlock } from "@/components/ProvenanceBlock";
import { SupersessionChain } from "@/components/SupersessionChain";
import { VerificationBadge } from "@/components/VerificationBadge";

export const revalidate = 300;

// Record pages are permanent URLs (records are immutable); rendered on first
// demand, then ISR-cached — empty generateStaticParams opts into that path.
export function generateStaticParams(): Array<{ id: string }> {
  return [];
}

interface Params {
  params: Promise<{ id: string }>;
}

async function fetchRecordOr404(id: string): Promise<RecordDetail> {
  try {
    return await getRecord(id);
  } catch (error) {
    if (error instanceof ApiError && (error.status === 404 || error.status === 400)) {
      notFound();
    }
    throw error;
  }
}

export async function generateMetadata({ params }: Params): Promise<Metadata> {
  const { id } = await params;
  const detail = await fetchRecordOr404(id);
  return {
    title: `${detail.record.asset_description_raw} — disclosure record`,
    description: `Disclosure record with provenance: ${detail.record.asset_description_raw}.`,
  };
}

// Neutral as-filed phrasing (design §7.5): "disclosed a purchase of …".
function disclosureSentence(detail: RecordDetail): string {
  const { record } = detail;
  switch (record.record_type) {
    case "transaction": {
      const verb =
        record.side === "buy"
          ? "a purchase of"
          : record.side === "sell"
            ? "a sale of"
            : record.side === "exchange"
              ? "an exchange of"
              : "a transaction in";
      return `disclosed ${verb}`;
    }
    case "holding":
      return "disclosed a holding of";
    case "interest":
      return "disclosed an interest in";
    case "change_notification":
      return "notified a change concerning";
  }
}

export default async function RecordPage({ params }: Params) {
  const { id } = await params;
  const detail = await fetchRecordOr404(id);
  const { record, provenance, supersedes, superseded_by } = detail;
  const politician = await politicianProfile(record.politician_id);
  const latestCorrection = superseded_by.at(-1);

  return (
    <>
      <section className="record-head">
        <p className="kind">
          <Link href={`/p/${politician.id}`}>{politician.canonical_name}</Link>{" "}
          {disclosureSentence(detail)}
        </p>
        <h1>{record.asset_description_raw}</h1>
        <p>
          <VerificationBadge state={record.verification_state} />{" "}
          {record.extraction_confidence !== null &&
          record.extraction_confidence !== undefined &&
          record.extraction_confidence < 1 ? (
            <span className="muted" data-testid="confidence">
              extraction confidence {formatConfidence(record.extraction_confidence)}
            </span>
          ) : null}
        </p>
        {latestCorrection ? (
          <p className="notice">
            A later record supersedes this one:{" "}
            <Link href={`/r/${latestCorrection.id}`}>
              {latestCorrection.asset_description_raw}
            </Link>
            .
          </p>
        ) : null}
      </section>

      <section className="record-facts" aria-label="Record fields">
        <dl className="facts">
          <dt>Declared value</dt>
          <dd className="cell-value">
            {record.value ? formatValueInterval(record.value) : "Not declared"}
          </dd>

          {record.transaction_date ? (
            <>
              <dt>Transaction date</dt>
              <dd>{formatDate(record.transaction_date)}</dd>
            </>
          ) : null}
          {record.notified_date ? (
            <>
              <dt>Notified date</dt>
              <dd>{formatDate(record.notified_date)}</dd>
            </>
          ) : null}
          {record.as_of_date ? (
            <>
              <dt>As-of date</dt>
              <dd>{formatDate(record.as_of_date)}</dd>
            </>
          ) : null}

          <dt>Type</dt>
          <dd>
            {record.record_type.replaceAll("_", " ")}
            {record.side ? ` · ${record.side}` : null}
          </dd>

          <dt>Asset class</dt>
          <dd>{record.asset_class.replaceAll("_", " ")}</dd>

          <dt>Owner</dt>
          <dd>{record.owner ?? "unknown"}</dd>

          <dt>Instrument</dt>
          <dd>
            {record.instrument_id ? (
              <span className="mono">{record.instrument_id}</span>
            ) : (
              <span className="muted">
                Not resolved — left unlinked rather than guessed
              </span>
            )}
          </dd>

          <dt>Extracted by</dt>
          <dd className="mono">{record.extracted_by}</dd>

          <dt>Fingerprint</dt>
          <dd className="mono">{record.fingerprint}</dd>

          <dt>Record ID</dt>
          <dd className="mono">{record.id}</dd>
        </dl>

        <details className="payload">
          <summary>Regime payload (as extracted)</summary>
          <pre>{JSON.stringify(record.details, null, 2)}</pre>
        </details>
      </section>

      <ProvenanceBlock provenance={provenance} />

      <section aria-label="Correction history">
        <h2>Correction history</h2>
        <SupersessionChain supersedes={supersedes} supersededBy={superseded_by} />
      </section>
    </>
  );
}
