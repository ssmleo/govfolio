import type { Metadata } from "next";

import { ProfileView, fetchProfileOr404 } from "../../profile-view";

export const revalidate = 300;

// On-demand ISR (see /p/[id]/page.tsx).
export function generateStaticParams(): Array<{ id: string; cursor: string }> {
  return [];
}

interface Params {
  params: Promise<{ id: string; cursor: string }>;
}

export async function generateMetadata({ params }: Params): Promise<Metadata> {
  const { id } = await params;
  const profile = await fetchProfileOr404(id);
  return {
    title: `${profile.canonical_name} — financial disclosures (earlier records)`,
    description: `Financial-disclosure records filed by ${profile.canonical_name}, with sources.`,
  };
}

export default async function PoliticianTimelinePage({ params }: Params) {
  const { id, cursor } = await params;
  return <ProfileView id={id} cursor={cursor} />;
}
