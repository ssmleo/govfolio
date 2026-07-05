import type { Metadata } from "next";

import { ProfileView, fetchProfileOr404 } from "./profile-view";

export const revalidate = 300;

// Profiles are rendered on first demand, then ISR-cached: an empty
// generateStaticParams opts the route into the static/ISR path.
export function generateStaticParams(): Array<{ id: string }> {
  return [];
}

interface Params {
  params: Promise<{ id: string }>;
}

export async function generateMetadata({ params }: Params): Promise<Metadata> {
  const { id } = await params;
  const profile = await fetchProfileOr404(id);
  return {
    title: `${profile.canonical_name} — financial disclosures`,
    description: `Financial-disclosure records filed by ${profile.canonical_name}, with sources.`,
  };
}

export default async function PoliticianPage({ params }: Params) {
  const { id } = await params;
  return <ProfileView id={id} />;
}
