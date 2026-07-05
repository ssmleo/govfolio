"use client";

import { useRouter } from "next/navigation";
import Link from "next/link";
import { useState } from "react";

import type {
  AssetClass,
  CorrectedPayload,
  Currency,
  DisclosureRecord,
  Owner,
  RecordType,
  ResolveActionResult,
  ResolveInput,
  Side,
} from "@/lib/api";

// Reviewer actions (design §7.2): confirm / edit / reject. Every verdict goes
// through ONE door — the resolve endpoint onto pipeline promote — via the
// server action passed in as `action`; this panel never fabricates state.
// A 409 (someone else resolved first) is surfaced honestly and the server-
// rendered task state is reloaded.

/** Extractor tag for human corrections made through this console. */
const CORRECTION_EXTRACTOR = "review:web@1";

// Option maps are Record<Enum, label> so adding a contract enum member makes
// the compiler point here (same pattern as VerificationBadge's STATE_META).
const RECORD_TYPE_OPTIONS: Record<RecordType, string> = {
  transaction: "transaction",
  holding: "holding",
  interest: "interest",
  change_notification: "change notification",
};

const ASSET_CLASS_OPTIONS: Record<AssetClass, string> = {
  equity: "equity",
  bond: "bond",
  fund: "fund",
  option: "option",
  crypto: "crypto",
  commodity: "commodity",
  real_estate: "real estate",
  private: "private",
  other: "other",
};

const SIDE_OPTIONS: Record<Side, string> = {
  buy: "buy",
  sell: "sell",
  exchange: "exchange",
};

const OWNER_OPTIONS: Record<Owner, string> = {
  self: "self",
  spouse: "spouse",
  dependent: "dependent",
  joint: "joint",
  unknown: "unknown",
};

const CURRENCY_OPTIONS: Record<Currency, string> = {
  EUR: "EUR",
  GBP: "GBP",
  USD: "USD",
};

function isKeyOf<T extends string>(map: Record<T, string>, value: string): value is T {
  return value in map;
}

function keysOf<T extends string>(map: Record<T, string>): T[] {
  return Object.keys(map) as T[];
}

/** Decimal-string check (invariant 7): bounds never pass through Number. */
const DECIMAL_RE = /^[0-9]+(\.[0-9]+)?$/;

export interface ResolvePanelProps {
  taskId: string;
  status: string;
  targetKind: string;
  targetId: string;
  record: DisclosureRecord | null;
  action: (taskId: string, input: ResolveInput) => Promise<ResolveActionResult>;
}

export function ResolvePanel({
  taskId,
  status,
  targetKind,
  targetId,
  record,
  action,
}: ResolvePanelProps) {
  const router = useRouter();

  const [reviewer, setReviewer] = useState("");
  const [note, setNote] = useState("");
  const [editOpen, setEditOpen] = useState(false);
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ResolveActionResult | null>(null);

  // Edit form state, seeded with the record's CURRENT values. Money bounds
  // stay strings end to end (invariant 7).
  const [description, setDescription] = useState(record?.asset_description_raw ?? "");
  const [recordType, setRecordType] = useState<RecordType>(
    record?.record_type ?? "transaction",
  );
  const [assetClass, setAssetClass] = useState<AssetClass>(record?.asset_class ?? "other");
  const [side, setSide] = useState<Side | "">(record?.side ?? "");
  const [owner, setOwner] = useState<Owner | "">(record?.owner ?? "");
  const [transactionDate, setTransactionDate] = useState(record?.transaction_date ?? "");
  const [asOfDate, setAsOfDate] = useState(record?.as_of_date ?? "");
  const [notifiedDate, setNotifiedDate] = useState(record?.notified_date ?? "");
  const [valueLow, setValueLow] = useState(record?.value?.low ?? "");
  const [valueHigh, setValueHigh] = useState(record?.value?.high ?? "");
  const [currency, setCurrency] = useState<Currency>(record?.value?.currency ?? "USD");
  const [instrumentId, setInstrumentId] = useState(record?.instrument_id ?? "");
  const [regimeCode, setRegimeCode] = useState("");
  const [detailsText, setDetailsText] = useState(
    JSON.stringify(record?.details ?? {}, null, 2),
  );

  if (!record) {
    return (
      <section className="resolve-panel" aria-label="Actions">
        <h2>Actions</h2>
        <p className="muted" data-testid="no-adjudication">
          This task targets {targetKind} {targetId} — this console adjudicates
          disclosure records only; the resolve path fails closed for other targets.
        </p>
      </section>
    );
  }

  async function submit(input: ResolveInput): Promise<void> {
    setPending(true);
    setError(null);
    try {
      const outcome = await action(taskId, input);
      if (outcome.kind === "error") {
        setError(`${outcome.code}: ${outcome.message}`);
        return;
      }
      setResult(outcome);
      // Reload the server-rendered task state (status, audit log) — for a
      // conflict this is exactly "handle honestly: show what really happened".
      router.refresh();
    } finally {
      setPending(false);
    }
  }

  function sharedInputOrError(): { reviewer: string; note: string | null } | null {
    if (reviewer.trim() === "") {
      setError("Reviewer name is required.");
      return null;
    }
    return { reviewer: reviewer.trim(), note: note.trim() === "" ? null : note.trim() };
  }

  function submitVerdict(verdict: "confirm" | "reject"): void {
    const shared = sharedInputOrError();
    if (!shared) {
      return;
    }
    void submit({ ...shared, verdict });
  }

  function submitEdit(event: React.FormEvent<HTMLFormElement>): void {
    event.preventDefault();
    const shared = sharedInputOrError();
    if (!shared) {
      return;
    }
    if (regimeCode.trim() === "") {
      setError("Regime code is required for an edit (details-registry arm, e.g. us_house).");
      return;
    }
    const low = valueLow.trim();
    const high = valueHigh.trim();
    if ((low !== "" && !DECIMAL_RE.test(low)) || (high !== "" && !DECIMAL_RE.test(high))) {
      setError("Declared value bounds must be decimal strings, e.g. 15001.00.");
      return;
    }
    if (low === "" && high !== "") {
      setError("A declared value needs a low bound (high alone is not a band).");
      return;
    }
    let details: CorrectedPayload["details"];
    try {
      const parsed: unknown = JSON.parse(detailsText);
      if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) {
        throw new Error("not an object");
      }
      details = parsed as CorrectedPayload["details"];
    } catch {
      setError("Regime payload must be a JSON object.");
      return;
    }
    const corrected: CorrectedPayload = {
      instrument_id: instrumentId.trim() === "" ? null : instrumentId.trim(),
      asset_description_raw: description,
      record_type: recordType,
      asset_class: assetClass,
      side: side === "" ? null : side,
      transaction_date: transactionDate === "" ? null : transactionDate,
      as_of_date: asOfDate === "" ? null : asOfDate,
      notified_date: notifiedDate === "" ? null : notifiedDate,
      value: low === "" ? null : { low, high: high === "" ? null : high, currency },
      owner: owner === "" ? null : owner,
      // A human correction, not an extractor guess.
      extraction_confidence: null,
      extracted_by: CORRECTION_EXTRACTOR,
      details,
    };
    void submit({ ...shared, verdict: "edit", regime_code: regimeCode.trim(), corrected });
  }

  if (result?.kind === "applied") {
    return (
      <section className="resolve-panel" aria-label="Actions">
        <h2>Actions</h2>
        <div className="notice notice-applied" data-testid="resolve-outcome">
          <p>Verdict applied.</p>
          <ul>
            <li>
              Adjudicated record:{" "}
              <Link className="mono" href={`/r/${result.recordId}`}>
                {result.recordId}
              </Link>
            </li>
            {result.supersedingRecordId ? (
              <li>
                Superseding correction:{" "}
                <Link
                  className="mono"
                  href={`/r/${result.supersedingRecordId}`}
                  data-testid="superseding-link"
                >
                  {result.supersedingRecordId}
                </Link>
              </li>
            ) : null}
          </ul>
        </div>
      </section>
    );
  }

  if (result?.kind === "conflict") {
    return (
      <section className="resolve-panel" aria-label="Actions">
        <h2>Actions</h2>
        <div className="notice" data-testid="resolve-conflict">
          Already resolved: {result.message} Nothing was changed by this attempt —
          the task state and audit log below show what actually happened.
        </div>
      </section>
    );
  }

  if (status !== "open") {
    return (
      <section className="resolve-panel" aria-label="Actions">
        <h2>Actions</h2>
        <p className="muted" data-testid="task-closed">
          This task is {status}; adjudication is closed. The audit log below is the
          record of what happened.
        </p>
      </section>
    );
  }

  return (
    <section className="resolve-panel" aria-label="Actions">
      <h2>Actions</h2>
      {error ? (
        <p className="notice notice-error" role="alert" data-testid="resolve-error">
          {error}
        </p>
      ) : null}

      <div className="resolve-shared">
        <label>
          Reviewer
          <input
            type="text"
            name="reviewer"
            value={reviewer}
            onChange={(event) => setReviewer(event.target.value)}
            placeholder="your name — recorded in the audit log (accounts land in goal 050)"
          />
        </label>
        <label>
          Note
          <textarea
            name="note"
            value={note}
            onChange={(event) => setNote(event.target.value)}
            placeholder="optional; recorded verbatim in the audit log"
            rows={2}
          />
        </label>
      </div>

      <div className="resolve-actions">
        <button type="button" onClick={() => submitVerdict("confirm")} disabled={pending}>
          Confirm
        </button>
        <button type="button" onClick={() => submitVerdict("reject")} disabled={pending}>
          Reject
        </button>
        <button
          type="button"
          onClick={() => setEditOpen((open) => !open)}
          aria-expanded={editOpen}
        >
          {editOpen ? "Close edit form" : "Edit…"}
        </button>
      </div>

      {editOpen ? (
        <form className="edit-form" onSubmit={submitEdit} aria-label="Correction form">
          <p className="muted">
            An edit inserts a superseding corrected record; the original is never
            updated. Fields are seeded with the current values.
          </p>

          <label>
            Asset description (as filed)
            <input
              type="text"
              name="asset_description_raw"
              value={description}
              onChange={(event) => setDescription(event.target.value)}
            />
          </label>

          <div className="field-row">
            <label>
              Record type
              <select
                name="record_type"
                value={recordType}
                onChange={(event) => {
                  const next = event.target.value;
                  if (isKeyOf(RECORD_TYPE_OPTIONS, next)) {
                    setRecordType(next);
                  }
                }}
              >
                {keysOf(RECORD_TYPE_OPTIONS).map((key) => (
                  <option key={key} value={key}>
                    {RECORD_TYPE_OPTIONS[key]}
                  </option>
                ))}
              </select>
            </label>

            <label>
              Asset class
              <select
                name="asset_class"
                value={assetClass}
                onChange={(event) => {
                  const next = event.target.value;
                  if (isKeyOf(ASSET_CLASS_OPTIONS, next)) {
                    setAssetClass(next);
                  }
                }}
              >
                {keysOf(ASSET_CLASS_OPTIONS).map((key) => (
                  <option key={key} value={key}>
                    {ASSET_CLASS_OPTIONS[key]}
                  </option>
                ))}
              </select>
            </label>

            <label>
              Side
              <select
                name="side"
                value={side}
                onChange={(event) => {
                  const next = event.target.value;
                  setSide(next !== "" && isKeyOf(SIDE_OPTIONS, next) ? next : "");
                }}
              >
                <option value="">—</option>
                {keysOf(SIDE_OPTIONS).map((key) => (
                  <option key={key} value={key}>
                    {SIDE_OPTIONS[key]}
                  </option>
                ))}
              </select>
            </label>

            <label>
              Owner
              <select
                name="owner"
                value={owner}
                onChange={(event) => {
                  const next = event.target.value;
                  setOwner(next !== "" && isKeyOf(OWNER_OPTIONS, next) ? next : "");
                }}
              >
                <option value="">—</option>
                {keysOf(OWNER_OPTIONS).map((key) => (
                  <option key={key} value={key}>
                    {OWNER_OPTIONS[key]}
                  </option>
                ))}
              </select>
            </label>
          </div>

          <div className="field-row">
            <label>
              Transaction date
              <input
                type="date"
                name="transaction_date"
                value={transactionDate}
                onChange={(event) => setTransactionDate(event.target.value)}
              />
            </label>
            <label>
              As-of date
              <input
                type="date"
                name="as_of_date"
                value={asOfDate}
                onChange={(event) => setAsOfDate(event.target.value)}
              />
            </label>
            <label>
              Notified date
              <input
                type="date"
                name="notified_date"
                value={notifiedDate}
                onChange={(event) => setNotifiedDate(event.target.value)}
              />
            </label>
          </div>

          <div className="field-row">
            <label>
              Value low
              <input
                type="text"
                inputMode="decimal"
                name="value_low"
                value={valueLow}
                onChange={(event) => setValueLow(event.target.value)}
                placeholder="decimal string, e.g. 1001.00"
              />
            </label>
            <label>
              Value high
              <input
                type="text"
                inputMode="decimal"
                name="value_high"
                value={valueHigh}
                onChange={(event) => setValueHigh(event.target.value)}
                placeholder="empty = open-ended"
              />
            </label>
            <label>
              Currency
              <select
                name="currency"
                value={currency}
                onChange={(event) => {
                  const next = event.target.value;
                  if (isKeyOf(CURRENCY_OPTIONS, next)) {
                    setCurrency(next);
                  }
                }}
              >
                {keysOf(CURRENCY_OPTIONS).map((key) => (
                  <option key={key} value={key}>
                    {CURRENCY_OPTIONS[key]}
                  </option>
                ))}
              </select>
            </label>
          </div>

          <div className="field-row">
            <label>
              Instrument ID
              <input
                type="text"
                name="instrument_id"
                value={instrumentId}
                onChange={(event) => setInstrumentId(event.target.value)}
                placeholder="empty = unlinked (never guess)"
              />
            </label>
            <label>
              Regime code
              <input
                type="text"
                name="regime_code"
                value={regimeCode}
                onChange={(event) => setRegimeCode(event.target.value)}
                placeholder="details-registry arm, e.g. us_house"
              />
            </label>
          </div>

          <label>
            Regime payload (details, JSON)
            <textarea
              name="details"
              value={detailsText}
              onChange={(event) => setDetailsText(event.target.value)}
              rows={10}
              className="mono"
            />
          </label>

          <button type="submit" disabled={pending}>
            Submit correction
          </button>
        </form>
      ) : null}
    </section>
  );
}
