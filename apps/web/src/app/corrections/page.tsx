import type { Metadata } from "next";

import { CorrectionsView } from "./corrections-view";

// Rendered live (like the review queue): a corrections log is a trust surface,
// so a just-recorded correction must appear without ISR-cache lag. Visibility
// is still governed by the free-tier delay in the shared record evaluator.
export const dynamic = "force-dynamic";

export const metadata: Metadata = {
  title: "Corrections",
  description:
    "Disclosure records we have corrected, each linked to the earlier record it supersedes.",
};

export default function CorrectionsPage() {
  return <CorrectionsView />;
}
