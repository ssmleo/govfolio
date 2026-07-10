import type { Metadata } from "next";
import Link from "next/link";

import { listJurisdictions } from "@/lib/api";
import { RegimeTable } from "@/components/RegimeTable";

export const revalidate = 3600;

export const metadata: Metadata = {
  title: "Jurisdictions — disclosure scorecard",
  description:
    "Disclosure regimes by jurisdiction: what is disclosed, how precisely, on what cadence, and with what statutory lag.",
};

export default async function JurisdictionsPage() {
  const jurisdictions = await listJurisdictions();
  return (
    <>
      <section className="profile-head">
        <h1>Jurisdictions</h1>
        <p className="muted">
          One row per disclosure regime: type, value precision, filing cadence,
          and the statutory lag between an event and its publication.
        </p>
      </section>
      {jurisdictions.length === 0 ? (
        <p className="empty">No jurisdictions published yet.</p>
      ) : (
        jurisdictions.map((jurisdiction) => (
          <section
            key={jurisdiction.id}
            className="jurisdiction"
            aria-label={jurisdiction.name}
          >
            <h2>
              <Link href={`/jurisdictions/${encodeURIComponent(jurisdiction.id)}`}>
                {jurisdiction.name}
              </Link>
            </h2>
            <p className="jurisdiction-meta">
              {jurisdiction.level}
              {jurisdiction.iso_code ? ` · ISO ${jurisdiction.iso_code}` : null}
              {` · ${jurisdiction.regimes.length} regime`}
              {jurisdiction.regimes.length === 1 ? "" : "s"}
            </p>
            <div className="table-scroll">
              <RegimeTable regimes={jurisdiction.regimes} />
            </div>
          </section>
        ))
      )}
    </>
  );
}
