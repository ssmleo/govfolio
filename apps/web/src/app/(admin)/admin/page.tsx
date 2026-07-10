import Link from "next/link";

import type { AdminFrozenRegime, AdminOverview, AdminQueueDepths, Jurisdiction } from "@/lib/api";
import { ApiError, adminOverview, listJurisdictions } from "@/lib/api";
import { Card } from "@/components/admin/ui/Card";
import { Badge } from "@/components/admin/ui/Badge";
import { Stat } from "@/components/admin/ui/Stat";
import { Table, type TableColumn } from "@/components/admin/ui/Table";
import { CoverageHeatmap, type CoverageCell } from "@/components/admin/CoverageHeatmap";
import { Unavailable } from "@/components/admin/Unavailable";

// The landing page (goal 091 phase 2): "healthy? covered? what's left?"
// answerable in five seconds. SentinelTicker (mounted once in the shared
// layout) already covers the ambient poll — this page is the deeper,
// static-per-request read of the same snapshot plus the world coverage
// hero.
export const dynamic = "force-dynamic";

function formatUtc(iso: string): string {
  return new Date(iso).toUTCString();
}

function queueTone(count: number, tone?: "warning" | "danger"): "warning" | "danger" | undefined {
  return tone !== undefined && count > 0 ? tone : undefined;
}

const QUEUE_TILES: ReadonlyArray<{
  key: keyof AdminQueueDepths;
  label: string;
  toneWhenPositive?: "warning" | "danger";
}> = [
  { key: "outbox_undispatched", label: "outbox undispatched" },
  { key: "review_open", label: "review open" },
  { key: "drift_open", label: "drift open", toneWhenPositive: "warning" },
  { key: "sample_pending", label: "sample pending" },
  { key: "delivery_dlq", label: "delivery dead", toneWhenPositive: "danger" },
  { key: "usage_unbilled", label: "usage unbilled" },
];

const FROZEN_COLUMNS: ReadonlyArray<TableColumn<AdminFrozenRegime>> = [
  {
    key: "regime_code",
    header: "regime",
    render: (row) => <span className="adm-num">{row.regime_code}</span>,
  },
  {
    key: "frozen_kind",
    header: "kind",
    render: (row) =>
      row.frozen_kind !== null && row.frozen_kind !== undefined ? (
        <Badge variant="danger">{row.frozen_kind}</Badge>
      ) : (
        <span className="adm-muted">—</span>
      ),
  },
  {
    key: "frozen_at",
    header: "since",
    render: (row) =>
      row.frozen_at !== null && row.frozen_at !== undefined ? (
        <span className="adm-num text-xs adm-muted">{formatUtc(row.frozen_at)}</span>
      ) : (
        <span className="adm-muted">—</span>
      ),
  },
];

const OTHER_PAGES: ReadonlyArray<{ href: string; label: string; description: string }> = [
  { href: "/admin/coverage", label: "Coverage", description: "full world map, blocked jurisdictions" },
  { href: "/admin/backfill", label: "Backfill", description: "historical backfill runs and budget" },
  { href: "/admin/pipeline", label: "Pipeline", description: "stage funnel and failed runs" },
  { href: "/admin/quality", label: "Quality", description: "precision, drift, unlinked instruments" },
  { href: "/admin/storage", label: "Storage", description: "table sizes, retention, idempotency" },
  { href: "/admin/serving", label: "Serving", description: "API usage, alert latency, deliveries" },
  { href: "/admin/infra", label: "Infra", description: "budget ceiling and terraform mirror" },
  { href: "/admin/loop", label: "Loop", description: "agent loop, goals, and commits" },
];

export default async function AdminOverviewPage() {
  let data: AdminOverview;
  let jurisdictions: Jurisdiction[];
  try {
    [data, jurisdictions] = await Promise.all([adminOverview(), listJurisdictions()]);
  } catch (error) {
    if (error instanceof ApiError && (error.status === 401 || error.status === 403 || error.status === 503)) {
      return <Unavailable reason={error.message} />;
    }
    throw error;
  }

  const cells: CoverageCell[] = jurisdictions.map((j) => ({
    id: j.id,
    name: j.name,
    phase: j.coverage_phase,
  }));
  const liveCount = cells.filter((c) => c.phase === "live").length;
  const frozenCount = data.frozen_regimes.length;

  return (
    <div className="mx-auto flex max-w-5xl flex-col gap-4 px-4 py-6">
      <section className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <p className="adm-eyebrow mb-1">Mission control</p>
          <h1>Overview</h1>
          <p className="mt-1 max-w-2xl text-sm adm-muted">
            Coverage, queues, and runs across every disclosure regime — is anything broken, and
            what&rsquo;s left to build.
          </p>
        </div>
        <div className="flex flex-col items-end gap-1">
          <p className="adm-num text-xs adm-muted">as of {formatUtc(data.generated_at)}</p>
          <p className="adm-num text-xs adm-muted">
            sentinel last checked{" "}
            {data.last_sentinel_check !== null && data.last_sentinel_check !== undefined
              ? formatUtc(data.last_sentinel_check)
              : "never"}
          </p>
        </div>
      </section>

      <Card
        eyebrow="World coverage"
        title={`${liveCount} / ${cells.length} jurisdictions live`}
        action={
          <Link href="/admin/coverage" className="text-sm font-semibold no-underline">
            full map →
          </Link>
        }
      >
        <Link href="/admin/coverage" aria-label="Open the full coverage map" className="block">
          <CoverageHeatmap jurisdictions={cells} size="compact" />
        </Link>
      </Card>

      <div style={frozenCount > 0 ? { borderLeft: "3px solid var(--adm-danger-rule)" } : undefined}>
        <Card
          eyebrow="Sentinel"
          title="Frozen regimes"
          action={
            <Badge variant={frozenCount > 0 ? "danger" : "success"}>
              {frozenCount > 0 ? `${frozenCount} frozen` : "none frozen"}
            </Badge>
          }
        >
          <Table
            columns={FROZEN_COLUMNS}
            rows={data.frozen_regimes}
            getRowKey={(row) => row.regime_code}
            emptyMessage="No frozen regimes — every adapter publishing normally."
          />
        </Card>
      </div>

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-[2fr_1fr]">
        <Card eyebrow="B5" title="Queue depths">
          <div className="grid grid-cols-2 gap-4 sm:grid-cols-3">
            {QUEUE_TILES.map((tile) => (
              <Stat
                key={tile.key}
                label={tile.label}
                value={data.queue_depths[tile.key]}
                tone={queueTone(data.queue_depths[tile.key], tile.toneWhenPositive)}
              />
            ))}
          </div>
        </Card>

        <Card eyebrow="Trailing 24h" title="Pipeline runs">
          <div className="grid grid-cols-3 gap-4">
            <Stat
              label="running"
              value={data.runs_24h.running}
              tone={data.runs_24h.running > 0 ? "info" : undefined}
            />
            <Stat
              label="succeeded"
              value={data.runs_24h.succeeded}
              tone={data.runs_24h.succeeded > 0 ? "success" : undefined}
            />
            <Stat
              label="failed"
              value={data.runs_24h.failed}
              tone={data.runs_24h.failed > 0 ? "danger" : undefined}
            />
          </div>
        </Card>
      </div>

      <section>
        <p className="adm-eyebrow mb-2">More instruments</p>
        <ul className="grid list-none grid-cols-2 gap-2 p-0 sm:grid-cols-4">
          {OTHER_PAGES.map((page) => (
            <li key={page.href}>
              <Link
                href={page.href}
                className="block rounded-sm border border-[var(--adm-rule)] bg-[var(--adm-surface)] px-3 py-2 no-underline hover:border-[var(--adm-rule-strong)]"
              >
                <span className="block text-sm font-semibold text-[var(--adm-ink)]">{page.label}</span>
                <span className="block text-xs adm-muted">{page.description}</span>
              </Link>
            </li>
          ))}
        </ul>
      </section>
    </div>
  );
}
