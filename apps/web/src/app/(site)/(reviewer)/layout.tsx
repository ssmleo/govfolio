import type { Metadata } from "next";

// The reviewer console is an internal adjudication surface: kept out of every
// sitemap (goal 041) and explicitly noindexed here. Auth is goal 050 — until
// then `reviewer` is free text recorded in the audit log.
export const metadata: Metadata = {
  title: {
    default: "Review queue",
    template: "%s · govfolio review",
  },
  robots: { index: false, follow: false },
};

export default function ReviewerLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="reviewer">
      <p className="reviewer-banner">Reviewer console — internal; not indexed.</p>
      {children}
    </div>
  );
}
