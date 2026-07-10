import type { Metadata } from "next";

import { CorrectionsView } from "../../corrections-view";

// Rendered live (see /corrections/page.tsx); older pages are cursor-addressed.
export const dynamic = "force-dynamic";

export const metadata: Metadata = {
  title: "Corrections",
  description:
    "Disclosure records we have corrected, each linked to the earlier record it supersedes.",
};

interface Params {
  params: Promise<{ cursor: string }>;
}

export default async function CorrectionsCursorPage({ params }: Params) {
  const { cursor } = await params;
  return <CorrectionsView cursor={cursor} />;
}
