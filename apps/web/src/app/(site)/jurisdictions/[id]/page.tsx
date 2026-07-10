import type { Metadata } from "next";
import { notFound } from "next/navigation";

import type { Jurisdiction } from "@/lib/api";
import { listJurisdictions } from "@/lib/api";
import { RegimeTable } from "@/components/RegimeTable";

export const revalidate = 3600;

// On-demand ISR (see /p/[id]/page.tsx).
export function generateStaticParams(): Array<{ id: string }> {
  return [];
}

interface Params {
  params: Promise<{ id: string }>;
}

// The contract exposes jurisdictions as one joined listing (§6.1); a detail
// page filters it — no private endpoint behind the site.
async function fetchJurisdictionOr404(id: string): Promise<Jurisdiction> {
  const jurisdictions = await listJurisdictions();
  const match = jurisdictions.find((jurisdiction) => jurisdiction.id === id);
  if (!match) {
    notFound();
  }
  return match;
}

export async function generateMetadata({ params }: Params): Promise<Metadata> {
  const { id } = await params;
  const jurisdiction = await fetchJurisdictionOr404(id);
  return {
    title: `${jurisdiction.name} — disclosure regimes`,
    description: `Disclosure regimes of ${jurisdiction.name}: regime type, value precision, cadence, statutory lag.`,
  };
}

export default async function JurisdictionPage({ params }: Params) {
  const { id } = await params;
  const jurisdiction = await fetchJurisdictionOr404(id);
  return (
    <>
      <section className="profile-head">
        <h1>{jurisdiction.name}</h1>
        <p className="jurisdiction-meta">
          {jurisdiction.level}
          {jurisdiction.iso_code ? ` · ISO ${jurisdiction.iso_code}` : null}
          {jurisdiction.parent_id ? ` · part of ${jurisdiction.parent_id}` : null}
        </p>
      </section>
      <section aria-label="Disclosure regimes">
        <h2>Disclosure regimes</h2>
        <div className="table-scroll">
          <RegimeTable regimes={jurisdiction.regimes} />
        </div>
      </section>
    </>
  );
}
