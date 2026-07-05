import Link from "next/link";

import { listReviewTasks } from "@/lib/api";
import { QueueTable } from "@/components/reviewer/QueueTable";

// The review queue (design §7.2): open tasks in the API's ranking order
// (priority_score desc, created_at asc, id) — rendered verbatim, never
// re-sorted here. Always fresh: an adjudication queue must not be ISR-stale.
export const dynamic = "force-dynamic";

const STATUS_FILTERS = ["open", "resolved"] as const;

interface Search {
  searchParams: Promise<{ status?: string; cursor?: string }>;
}

function queueHref(status: string, cursor?: string): string {
  const params = new URLSearchParams();
  if (status !== "open") {
    params.set("status", status);
  }
  if (cursor !== undefined) {
    params.set("cursor", cursor);
  }
  const query = params.toString();
  return query === "" ? "/review" : `/review?${query}`;
}

export default async function ReviewQueuePage({ searchParams }: Search) {
  const { status = "open", cursor } = await searchParams;
  const page = await listReviewTasks({ status, cursor, limit: 50 });

  return (
    <>
      <section className="record-head">
        <h1>Review queue</h1>
        <p className="muted">
          Ranked by priority (impact × uncertainty), oldest first within a priority.
        </p>
      </section>

      <nav className="queue-filters" aria-label="Task status filter">
        {STATUS_FILTERS.map((filter) => (
          <Link
            key={filter}
            href={queueHref(filter)}
            aria-current={filter === status ? "page" : undefined}
          >
            {filter}
          </Link>
        ))}
      </nav>

      <QueueTable items={page.items} now={new Date()} />

      {page.next_cursor != null ? (
        <nav className="pager" aria-label="Pagination">
          <Link href={queueHref(status, page.next_cursor)} rel="next">
            Next page →
          </Link>
        </nav>
      ) : null}
    </>
  );
}
