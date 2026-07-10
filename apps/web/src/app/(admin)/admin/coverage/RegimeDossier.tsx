"use client";

import { useEffect } from "react";
import Link from "next/link";

import { CountsBar } from "@/components/admin/charts/CountsBar";
import type { RegimeDossierData } from "./dossier-data";

export interface RegimeDossierProps {
  /** `null` = closed. Passing a value opens the slide-over. */
  data: RegimeDossierData | null;
  onClose: () => void;
}

function TierBar({ label, value, max }: { label: string; value: number | null; max: number }) {
  const pct = value !== null && max > 0 ? Math.round((value / max) * 100) : 0;
  return (
    <div className="flex flex-col gap-1">
      <div className="flex items-baseline justify-between text-xs text-[var(--adm-muted)]">
        <span>{label}</span>
        <span className="adm-num">{value === null ? "—" : value.toLocaleString()}</span>
      </div>
      <div className="h-1.5 w-full overflow-hidden rounded-full bg-[var(--adm-rule)]">
        <div
          className="h-full bg-[var(--adm-accent)]"
          style={{ width: value === null ? "0%" : `${pct}%` }}
        />
      </div>
    </div>
  );
}

// A 520px slide-over: one regime's facts, tier composition, gold-by-year
// density, integrity/freshness notes, the synthesized regime note, and the
// honest "not observable" politeness caveat — everything `dossier-data.ts`
// assembled, with zero data logic of its own. Closes on the × button,
// backdrop click, or Escape (unconditional — this surface has no text
// inputs to guard, unlike the sidebar's digit-shortcut listener).
export function RegimeDossier({ data, onClose }: RegimeDossierProps) {
  const open = data !== null;

  useEffect(() => {
    if (!open) return;
    const handler = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div className="fixed inset-0 z-50 flex justify-end">
      <div
        aria-hidden="true"
        onClick={onClose}
        className="absolute inset-0 bg-black/50"
      />
      <aside
        aria-label={`Regime dossier — ${data.title}`}
        className="relative flex h-full w-[520px] max-w-full flex-col gap-5 overflow-y-auto border-l border-[var(--adm-rule-strong)] bg-[var(--adm-surface)] px-5 py-5 shadow-[var(--adm-card-shadow)]"
      >
        <div className="flex items-start justify-between gap-3">
          <div>
            <p className="adm-eyebrow mb-1">Regime dossier</p>
            <h2 className="text-lg font-semibold">{data.title}</h2>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close dossier"
            className="rounded-sm border border-[var(--adm-rule-strong)] px-2 py-1 text-sm text-[var(--adm-muted)] hover:bg-[var(--adm-surface-sunken)]"
          >
            ×
          </button>
        </div>

        <section className="grid grid-cols-2 gap-x-4 gap-y-3">
          {data.facts.map((fact) => (
            <div key={fact.label} className="flex flex-col gap-0.5">
              <p className="adm-eyebrow">{fact.label}</p>
              <p className="text-sm text-[var(--adm-ink)]">{fact.value}</p>
            </div>
          ))}
        </section>

        <section className="flex flex-col gap-2">
          <p className="adm-eyebrow">Tier composition</p>
          <TierBar label="bronze" value={data.tiers.bronze} max={data.tiers.maxTier} />
          <TierBar label="silver" value={data.tiers.silver} max={data.tiers.maxTier} />
          <TierBar label="gold" value={data.tiers.gold} max={data.tiers.maxTier} />
        </section>

        <section className="flex flex-col gap-2">
          <p className="adm-eyebrow">Gold records by year</p>
          {data.goldByYear.length > 0 ? (
            <CountsBar
              data={data.goldByYear}
              categoryKey="year"
              series={[{ key: "count", label: "gold records" }]}
              height={160}
            />
          ) : (
            <p className="text-sm text-[var(--adm-muted)]">No dated Gold records for this regime yet.</p>
          )}
        </section>

        <section className="flex flex-col gap-2">
          <p className="adm-eyebrow">Integrity</p>
          <p className="text-sm text-[var(--adm-ink)]">{data.integrityNote}</p>
          <p className="text-sm text-[var(--adm-ink)]">{data.freshnessNote}</p>
        </section>

        {data.regimeNote !== null && (
          <section className="flex flex-col gap-1">
            <p className="adm-eyebrow">Regime note</p>
            <p className="text-sm italic text-[var(--adm-muted)]">{data.regimeNote}</p>
          </section>
        )}

        <section className="flex flex-col gap-1">
          <p className="adm-eyebrow">Politeness</p>
          <p className="text-xs text-[var(--adm-muted)]">{data.politenessNote}</p>
        </section>

        <footer className="mt-auto flex flex-col gap-2 border-t border-[var(--adm-rule)] pt-4">
          <p className="text-xs text-[var(--adm-muted)]">
            Adapter crate
            {data.adapterCrates.length === 1 ? "" : "s"}:{" "}
            {data.adapterCrates.length > 0
              ? data.adapterCrates.map((ref, i) => (
                  <span key={ref.regimeCode}>
                    {i > 0 && ", "}
                    <code className="adm-num text-xs">
                      {ref.crate !== null ? `crates/adapters/${ref.crate}` : `${ref.regimeCode} (unmapped)`}
                    </code>
                  </span>
                ))
              : "unbridged"}
          </p>
          <Link
            href="/admin/pipeline"
            className="inline-flex w-fit items-center gap-1.5 rounded-sm border border-[var(--adm-rule-strong)] bg-[var(--adm-surface-sunken)] px-3 py-1.5 text-sm font-semibold text-[var(--adm-ink)] no-underline hover:bg-[var(--adm-rule)]"
          >
            See full pipeline detail →
          </Link>
        </footer>
      </aside>
    </div>
  );
}
