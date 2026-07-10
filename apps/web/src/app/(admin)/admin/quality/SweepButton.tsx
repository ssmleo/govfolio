"use client";

import { useRouter } from "next/navigation";
import { useTransition } from "react";

export function SweepButton() {
  const router = useRouter();
  const [isPending, startTransition] = useTransition();

  const handleClick = () => {
    startTransition(() => {
      router.push("/admin/quality?sweep=br");
    });
  };

  return (
    <button
      onClick={handleClick}
      disabled={isPending}
      className="inline-flex w-fit items-center gap-1.5 rounded-sm border border-[var(--adm-rule-strong)] bg-[var(--adm-surface-sunken)] px-3 py-1.5 text-sm font-semibold text-[var(--adm-ink)] no-underline hover:bg-[var(--adm-rule)] disabled:opacity-60"
    >
      {isPending ? "Running..." : "Run br CPF collision sweep"}
    </button>
  );
}
