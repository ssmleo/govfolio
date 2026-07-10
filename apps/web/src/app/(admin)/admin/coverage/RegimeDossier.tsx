"use client";

import { Fragment, useEffect, useState } from "react";
import Link from "next/link";

import { Badge, stateVariant } from "@/components/admin/ui/Badge";
import { Progress } from "@/components/admin/ui/Progress";
import { YearBars } from "@/components/admin/charts/YearBars";
import type { RegimeDossierData } from "./dossier-data";

export interface RegimeDossierProps {
  /** `null` = closed. Passing a value opens the slide-over. */
  data: RegimeDossierData | null;
  onClose: () => void;
}

const TIER_ROWS: ReadonlyArray<{
  key: "bronze" | "silver" | "gold";
  label: string;
  color: string;
}> = [
  { key: "bronze", label: "Bronze", color: "var(--adm-series-bronze)" },
  { key: "silver", label: "Silver", color: "var(--adm-series-silver)" },
  { key: "gold", label: "Gold", color: "var(--adm-series-gold)" },
];

// A 520px slide-over (dc.html:1246-1304, panel styles 1885-1904): one
// regime's facts, tier composition, gold-by-year density, integrity/
// freshness/politeness notes, and the "open pipeline" footer. ALWAYS
// MOUNTED once opened for the first time — `cached` keeps the last
// non-null payload so closing plays the .38s slide-out instead of
// vanishing instantly (RegimeDossier.test.tsx's "never opened" case still
// renders nothing: `cached` starts `null` and stays `null` until the
// first non-null `data` arrives). `visibility` is delayed by the same
// .38s on close (not on open) so the panel is still hit-testable/visible
// to Playwright/screen readers through the whole slide, and `inert` keeps
// it out of the a11y tree and off the tab order while closed.
export function RegimeDossier({ data, onClose }: RegimeDossierProps) {
  const [cached, setCached] = useState<RegimeDossierData | null>(data);
  useEffect(() => {
    if (data !== null) setCached(data);
  }, [data]);

  const open = data !== null;

  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (event.key === "Escape" && open) onClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [open, onClose]);

  if (cached === null) return null;

  // "phase" and "jurisdiction" are promoted out of the facts grid: phase
  // becomes the header badge, jurisdiction becomes the subtitle line
  // (dc.html:1255-1257) — showing either twice would be genuinely
  // redundant, not just noisy.
  const phaseFact = cached.facts.find((f) => f.label === "phase");
  const jurisdictionFact = cached.facts.find((f) => f.label === "jurisdiction");
  const otherFacts = cached.facts.filter(
    (f) => f.label !== "phase" && f.label !== "jurisdiction",
  );
  const maxTier = Math.max(1, cached.tiers.maxTier);

  return (
    <>
      <div
        aria-hidden="true"
        onClick={onClose}
        style={{
          position: "fixed",
          inset: 0,
          zIndex: 40,
          background: "var(--adm-backdrop)",
          opacity: open ? 1 : 0,
          transition: "opacity .3s ease",
          pointerEvents: open ? "auto" : "none",
        }}
      />
      <aside
        aria-label={`Regime dossier — ${cached.title}`}
        inert={!open}
        style={{
          position: "fixed",
          top: 0,
          right: 0,
          bottom: 0,
          width: "var(--adm-dossier-w)",
          maxWidth: "100%",
          zIndex: 45,
          display: "flex",
          flexDirection: "column",
          background: "var(--adm-dossier-bg)",
          borderLeft: "1px solid var(--adm-gold-35)",
          boxShadow: "var(--adm-dossier-shadow)",
          transform: `translateX(${open ? 0 : "104%"})`,
          visibility: open ? "visible" : "hidden",
          transition: open
            ? "transform .38s cubic-bezier(.22,.61,.36,1)"
            : "transform .38s cubic-bezier(.22,.61,.36,1), visibility 0s .38s",
        }}
      >
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            gap: 12,
            padding: "18px 24px 0",
          }}
        >
          <p
            style={{
              margin: 0,
              fontSize: "10px",
              fontWeight: 700,
              letterSpacing: ".2em",
              textTransform: "uppercase",
              color: "var(--adm-accent)",
            }}
          >
            Regime dossier
          </p>
          <button
            type="button"
            onClick={onClose}
            aria-label="Close dossier"
            title="Close (esc)"
            className="grid h-7 w-7 place-items-center rounded-[2px] border border-[var(--adm-chip-border)] bg-transparent text-[15px] leading-none text-[var(--adm-muted)] hover:border-[var(--adm-gold-45)] hover:text-[var(--adm-accent-deep)]"
            style={{ transition: "color .15s, border-color .15s" }}
          >
            ×
          </button>
        </div>

        <div
          style={{
            padding: "10px 24px 26px",
            overflowY: "auto",
            overflowX: "hidden",
            flex: 1,
          }}
        >
          <h2 style={{ margin: 0, fontSize: "23px", lineHeight: 1.2 }}>{cached.title}</h2>
          {jurisdictionFact !== undefined && (
            <p style={{ margin: "4px 0 12px", color: "var(--adm-muted)" }}>
              {jurisdictionFact.value}
            </p>
          )}
          {phaseFact !== undefined && (
            <div style={{ display: "flex", gap: 8, alignItems: "center" }}>
              <Badge variant={stateVariant(phaseFact.value)}>{phaseFact.value}</Badge>
            </div>
          )}

          <div style={{ borderTop: "3px double var(--adm-gold-55)", marginTop: 18, paddingTop: 16 }}>
            <div
              style={{
                display: "grid",
                gridTemplateColumns: "max-content 1fr",
                gap: "9px 22px",
              }}
            >
              {otherFacts.map((fact) => (
                <Fragment key={fact.label}>
                  <p
                    style={{ margin: 0, fontSize: "11px", color: "var(--adm-meta)", paddingTop: 1 }}
                  >
                    {fact.label}
                  </p>
                  <p
                    className="adm-num"
                    style={{ margin: 0, fontSize: "12px", color: "var(--adm-heading)", overflowWrap: "anywhere" }}
                  >
                    {fact.value}
                  </p>
                </Fragment>
              ))}
            </div>
          </div>

          <div style={{ borderTop: "1px solid var(--adm-rule)", marginTop: 18, paddingTop: 16 }}>
            <p
              style={{
                margin: "0 0 12px",
                fontSize: "10px",
                fontWeight: 700,
                letterSpacing: ".16em",
                textTransform: "uppercase",
                color: "var(--adm-meta)",
              }}
            >
              Tier composition
            </p>
            <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
              {TIER_ROWS.map((row) => {
                const value = cached.tiers[row.key];
                const pct = value !== null && value !== undefined ? value / maxTier : 0;
                return (
                  <div key={row.key} style={{ display: "flex", alignItems: "center", gap: 12 }}>
                    <span
                      style={{
                        width: 58,
                        flexShrink: 0,
                        display: "inline-flex",
                        alignItems: "center",
                        gap: 6,
                        fontSize: "10px",
                        fontWeight: 700,
                        letterSpacing: ".1em",
                        textTransform: "uppercase",
                        color: "var(--adm-muted)",
                      }}
                    >
                      <span
                        style={{
                          width: 8,
                          height: 8,
                          borderRadius: "1px",
                          background: row.color,
                          display: "inline-block",
                        }}
                      />
                      {row.label}
                    </span>
                    <div style={{ flex: 1 }}>
                      <Progress value={pct} color={row.color} height={6} />
                    </div>
                    <span
                      className="adm-num"
                      style={{
                        width: 74,
                        flexShrink: 0,
                        textAlign: "right",
                        fontSize: "11.5px",
                        color: "var(--adm-text-secondary)",
                      }}
                    >
                      {value === null || value === undefined ? "—" : value.toLocaleString()}
                    </span>
                  </div>
                );
              })}
            </div>
          </div>

          <div style={{ borderTop: "1px solid var(--adm-rule)", marginTop: 18, paddingTop: 16 }}>
            <p
              style={{
                margin: "0 0 12px",
                fontSize: "10px",
                fontWeight: 700,
                letterSpacing: ".16em",
                textTransform: "uppercase",
                color: "var(--adm-meta)",
              }}
            >
              Gold records by year
            </p>
            {cached.goldByYear.length > 0 ? (
              <YearBars
                years={cached.goldByYear.map((p) => ({ year: p.year, value: p.count }))}
                firstLabel={cached.goldByYear[0]?.year ?? "—"}
                lastLabel={cached.goldByYear[cached.goldByYear.length - 1]?.year ?? "—"}
              />
            ) : (
              <p style={{ margin: 0, fontSize: "12.5px", color: "var(--adm-muted)" }}>
                No dated Gold records for this regime yet.
              </p>
            )}
          </div>

          <div style={{ borderTop: "1px solid var(--adm-rule)", marginTop: 18, paddingTop: 16 }}>
            <p className="adm-microlabel" style={{ margin: "0 0 6px" }}>
              Integrity
            </p>
            <p style={{ margin: 0, fontSize: "12.5px", color: "var(--adm-ink)" }}>
              {cached.integrityNote}
            </p>
            <p style={{ margin: "6px 0 0", fontSize: "12.5px", color: "var(--adm-ink)" }}>
              {cached.freshnessNote}
            </p>
          </div>

          {cached.regimeNote !== null && (
            <div style={{ borderTop: "1px solid var(--adm-rule)", marginTop: 18, paddingTop: 14 }}>
              <p className="adm-microlabel" style={{ margin: "0 0 4px" }}>
                Regime note
              </p>
              <p style={{ margin: 0, fontSize: "12px", fontStyle: "italic", color: "var(--adm-muted)" }}>
                {cached.regimeNote}
              </p>
            </div>
          )}

          <div style={{ borderTop: "1px solid var(--adm-rule)", marginTop: 18, paddingTop: 14 }}>
            <p className="adm-microlabel" style={{ margin: "0 0 4px" }}>
              Politeness
            </p>
            <p style={{ margin: 0, fontSize: "11.5px", color: "var(--adm-muted)" }}>
              {cached.politenessNote}
            </p>
          </div>

          <footer
            style={{
              marginTop: 20,
              display: "flex",
              flexDirection: "column",
              gap: 8,
              borderTop: "1px solid var(--adm-rule)",
              paddingTop: 14,
            }}
          >
            <p style={{ margin: 0, fontSize: "11.5px", color: "var(--adm-muted)" }}>
              Adapter crate{cached.adapterCrates.length === 1 ? "" : "s"}:{" "}
              {cached.adapterCrates.length > 0
                ? cached.adapterCrates.map((ref, i) => (
                    <span key={ref.regimeCode}>
                      {i > 0 && ", "}
                      <code className="adm-num" style={{ fontSize: "11.5px" }}>
                        {ref.crate !== null ? `crates/adapters/${ref.crate}` : `${ref.regimeCode} (unmapped)`}
                      </code>
                    </span>
                  ))
                : "unbridged"}
            </p>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 12 }}>
              <Link href="/admin/pipeline">See full pipeline detail →</Link>
            </div>
          </footer>
        </div>
      </aside>
    </>
  );
}
