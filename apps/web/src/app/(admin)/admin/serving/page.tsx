import type {
  AdminDeadDelivery,
  AdminDeliveryStatusChannel,
  AdminServing,
} from "@/lib/api";
import { ApiError, adminServing } from "@/lib/api";
import { Card } from "@/components/admin/ui/Card";
import { Stat } from "@/components/admin/ui/Stat";
import { Badge, stateVariant } from "@/components/admin/ui/Badge";
import { Table } from "@/components/admin/ui/Table";
import type { TableColumn } from "@/components/admin/ui/Table";
import { TrendArea } from "@/components/admin/charts/TrendArea";
import { Histogram } from "@/components/admin/charts/Histogram";
import { Unavailable } from "@/components/admin/Unavailable";

export const dynamic = "force-dynamic";

function formatCount(value: number): string {
  return value.toLocaleString("en-US");
}

function formatSeconds(value: number | null | undefined): string {
  if (value === null || value === undefined) {
    return "—";
  }
  return value < 60 ? `${value.toFixed(1)}s` : `${Math.round(value / 60)}m`;
}

function formatDay(day: string): string {
  return new Date(`${day}T00:00:00Z`).toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
    timeZone: "UTC",
  });
}

function formatDateTime(iso: string): string {
  const formatted = new Date(iso).toLocaleString("en-US", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
    timeZone: "UTC",
  });
  return `${formatted} UTC`;
}

function tierLabel(tier: string): string {
  return tier.charAt(0).toUpperCase() + tier.slice(1);
}

const statusChannelColumns: TableColumn<AdminDeliveryStatusChannel>[] = [
  {
    key: "status",
    header: "Status",
    render: (row) => <Badge variant={stateVariant(row.status)}>{row.status}</Badge>,
  },
  { key: "channel", header: "Channel", render: (row) => row.channel },
  { key: "count", header: "Deliveries", numeric: true, render: (row) => formatCount(row.count) },
];

const dlqColumns: TableColumn<AdminDeadDelivery>[] = [
  {
    key: "id",
    header: "ID",
    render: (row) => <span className="adm-num text-xs">{row.id}</span>,
  },
  { key: "channel", header: "Channel", render: (row) => row.channel },
  { key: "attempts", header: "Attempts", numeric: true, render: (row) => row.attempts },
  {
    key: "last_error",
    header: "Last error",
    render: (row) => (
      <span className="block max-w-[28rem] truncate" title={row.last_error ?? undefined}>
        {row.last_error ?? "—"}
      </span>
    ),
  },
  {
    key: "updated_at",
    header: "Updated",
    render: (row) => formatDateTime(row.updated_at),
  },
];

export default async function ServingPage() {
  let data: AdminServing;
  try {
    data = await adminServing();
  } catch (error) {
    if (error instanceof ApiError && (error.status === 401 || error.status === 403 || error.status === 503)) {
      return <Unavailable reason={error.message} />;
    }
    throw error;
  }

  const usageData = data.usage_by_day.map((row) => ({ x: formatDay(row.day), y: row.requests }));
  const endpointData = data.top_endpoints_7d.map((row) => ({
    bucket: row.endpoint,
    count: row.requests,
  }));
  const attemptsData = data.deliveries.attempts_histogram.map((row) => ({
    bucket: String(row.attempts),
    count: row.count,
  }));

  return (
    <div className="mx-auto flex w-full max-w-[1400px] flex-col gap-6 px-6 py-6">
      <section className="flex flex-col gap-1">
        <h1>Serving &amp; product</h1>
        <p className="text-xs adm-muted">Generated {formatDateTime(data.generated_at)}</p>
      </section>

      <Card eyebrow="F1 · usage" title="Usage">
        <div className="flex flex-col gap-6">
          <div>
            <div className="mb-2 flex items-baseline justify-between">
              <h3>Requests per day</h3>
              <span className="text-xs adm-muted">last 14 days</span>
            </div>
            {usageData.length > 0 ? (
              <TrendArea data={usageData} valueLabel="requests" height={200} />
            ) : (
              <p className="text-sm adm-muted">No requests in the last 14 days.</p>
            )}
          </div>
          <div>
            <div className="mb-2 flex items-baseline justify-between">
              <h3>Top endpoints</h3>
              <span className="text-xs adm-muted">last 7 days</span>
            </div>
            {endpointData.length > 0 ? (
              <Histogram data={endpointData} height={200} />
            ) : (
              <p className="text-sm adm-muted">No metered requests in the last 7 days.</p>
            )}
          </div>
        </div>
      </Card>

      <Card eyebrow="F1 · accounts" title="Accounts">
        <div className="grid grid-cols-2 gap-x-6 gap-y-5 sm:grid-cols-3 lg:grid-cols-6">
          {data.accounts.users_by_tier.map((row) => (
            <Stat key={row.tier} label={tierLabel(row.tier)} value={formatCount(row.users)} caption="users" />
          ))}
          <Stat label="Active keys" value={formatCount(data.accounts.active_keys)} />
          <Stat label="Revoked keys" value={formatCount(data.accounts.revoked_keys)} />
          <Stat label="Active subscriptions" value={formatCount(data.accounts.active_subscriptions)} />
        </div>
      </Card>

      <Card eyebrow="F2 · alert latency" title="Alert pipeline latency">
        <div className="flex flex-col gap-6">
          <div>
            <h3 className="mb-2">Dispatch (outbox → delivery)</h3>
            <div className="grid grid-cols-2 gap-x-6 gap-y-5 sm:grid-cols-3 lg:grid-cols-5">
              <Stat label="p50" value={formatSeconds(data.alert_latency.dispatch_p50_seconds)} />
              <Stat label="p90" value={formatSeconds(data.alert_latency.dispatch_p90_seconds)} />
              <Stat label="p99" value={formatSeconds(data.alert_latency.dispatch_p99_seconds)} />
              <Stat
                label="Dispatched"
                value={formatCount(data.alert_latency.dispatched_count)}
                caption="≥1s delta"
              />
              <Stat
                label="Pre-dispatched"
                value={formatCount(data.alert_latency.pre_dispatched_count)}
                caption="suppressed, <1s delta"
              />
            </div>
            <p className="mt-3 text-xs adm-muted">{data.alert_latency.note}</p>
          </div>
          <div>
            <h3 className="mb-2">Send (delivery created → sent)</h3>
            <div className="grid grid-cols-2 gap-x-6 gap-y-5 sm:grid-cols-3">
              <Stat label="p50" value={formatSeconds(data.alert_latency.send_p50_seconds)} />
              <Stat label="p90" value={formatSeconds(data.alert_latency.send_p90_seconds)} />
              <Stat label="Sent" value={formatCount(data.alert_latency.sent_count)} />
            </div>
          </div>
        </div>
      </Card>

      <Card eyebrow="F3 · deliveries" title="Delivery health">
        <div className="grid gap-6 lg:grid-cols-2">
          <div>
            <h3 className="mb-2">Status × channel</h3>
            <Table
              columns={statusChannelColumns}
              rows={data.deliveries.by_status_channel}
              getRowKey={(row) => `${row.status}-${row.channel}`}
              emptyMessage="No deliveries recorded."
            />
          </div>
          <div>
            <h3 className="mb-2">Attempts</h3>
            {attemptsData.length > 0 ? (
              <Histogram data={attemptsData} height={200} />
            ) : (
              <p className="text-sm adm-muted">No delivery attempts recorded.</p>
            )}
          </div>
        </div>
      </Card>

      <Card eyebrow="F3 · deliveries" title="Recent dead-letters">
        <Table
          columns={dlqColumns}
          rows={data.deliveries.recent_dead}
          getRowKey={(row) => row.id}
          emptyMessage="No dead-lettered deliveries."
        />
      </Card>

      <p className="border-t border-[var(--adm-rule)] pt-4 text-xs adm-muted">{data.latency_note}</p>
    </div>
  );
}
