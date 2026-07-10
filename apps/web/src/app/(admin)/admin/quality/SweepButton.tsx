"use client";

import { useRouter } from "next/navigation";
import { useTransition } from "react";

import { GhostButton } from "@/components/admin/ui/GhostButton";

// Idle explainer + the gold ghost trigger for the opt-in br CPF collision
// sweep (dc.html:776-782). While the ?sweep=br navigation is in flight the
// whole block swaps to the pulsing scan line — REAL transition state from
// useTransition around router.push, never a fake timer.
export function SweepButton() {
  const router = useRouter();
  const [isPending, startTransition] = useTransition();

  if (isPending) {
    return (
      <p
        style={{
          margin: "12px 0 0",
          fontFamily: "var(--adm-font-data)",
          fontSize: 12,
          color: "var(--adm-accent-deep)",
          animation: "gfPulse 1.2s ease-in-out infinite",
        }}
      >
        scanning br staged rows — comparing CPFs per politician…
      </p>
    );
  }

  return (
    <>
      <p style={{ margin: "12px 0 14px", fontSize: "12.5px", color: "var(--adm-muted)", maxWidth: 620 }}>
        Scans every br filing’s staged rows to compare CPFs per politician — a whole-dataset
        scan, not a cheap query. Zero rows is a pass; any row needs investigation.
      </p>
      <GhostButton
        onClick={() => {
          startTransition(() => {
            router.push("/admin/quality?sweep=br");
          });
        }}
      >
        Run collision sweep
      </GhostButton>
    </>
  );
}
