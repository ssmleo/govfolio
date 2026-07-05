import type { VerificationState } from "@/lib/api";

// Honest, visually distinct labels for the four publication states
// (design §7.1/§7.3). Color is never the only signal: label text differs.
const STATE_META: Record<VerificationState, { label: string; explains: string }> = {
  unverified: {
    label: "Unverified",
    explains: "Published as filed; not yet reviewed.",
  },
  verified: {
    label: "Verified",
    explains: "Checked against the source document.",
  },
  corrected: {
    label: "Corrected",
    explains: "A correction of an earlier record; the history is preserved.",
  },
  disputed: {
    label: "Disputed",
    explains: "Accuracy is contested; shown with that caveat.",
  },
};

export function VerificationBadge({ state }: { state: VerificationState }) {
  const meta = STATE_META[state];
  return (
    <span className={`badge badge-${state}`} data-state={state} title={meta.explains}>
      {meta.label}
    </span>
  );
}
