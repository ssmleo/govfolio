import Link from "next/link";
import { notFound } from "next/navigation";

import { ApiError, getRecord, listRecords } from "@/lib/api";
import { CorrectionsLog, type CorrectionItem } from "@/components/CorrectionsLog";

// The public corrections log (design §7.4; invariant 1 made visible): every
// Gold record we later corrected, read straight from the shared record grammar
// (`verification_state = corrected`) with no dedicated endpoint. It runs
// server-side and UNAUTHENTICATED — the free tier, correct for a public
// transparency log; corrections still inside the free delay simply aren't
// visible yet, exactly as everywhere else.
//
// Shared by /corrections (first page) and /corrections/from/[cursor] (older
// pages): records are immutable and ULID-ordered, so pagination lives in the
// path as a stable cursor URL (mirrors the timeline).
export async function CorrectionsView({ cursor }: { cursor?: string }) {
  let page;
  try {
    page = await listRecords(
      cursor === undefined
        ? { verification_state: "corrected" }
        : { verification_state: "corrected", cursor },
    );
  } catch (error) {
    if (error instanceof ApiError && error.status === 400) {
      notFound(); // malformed cursor in the URL
    }
    throw error;
  }

  // Pair each correction with the record it directly supersedes. The detail
  // endpoint already resolves the supersession chain (goal 040a); the
  // immediate predecessor is the one `supersedes_record_id` points at (the
  // last of the ascending ancestor chain), so "before -> after" is shown
  // without any new endpoint.
  const items: CorrectionItem[] = await Promise.all(
    page.items.map(async (record): Promise<CorrectionItem> => {
      const detail = await getRecord(record.id);
      const superseded =
        detail.supersedes.find((r) => r.id === record.supersedes_record_id) ??
        detail.supersedes.at(-1) ??
        null;
      return { correction: record, superseded };
    }),
  );

  return (
    <>
      <section className="profile-head">
        <h1>Corrections</h1>
        <p className="muted">
          Records we have corrected since publishing them. A correction is a new version
          that supersedes an earlier record; the original is never overwritten — it stays
          on file and is linked from every entry below.
        </p>
      </section>

      <section aria-label="Corrections log">
        <CorrectionsLog items={items} />
        <nav className="pager" aria-label="Corrections pages">
          {cursor !== undefined ? <Link href="/corrections">Newest corrections</Link> : null}
          {page.next_cursor !== null && page.next_cursor !== undefined ? (
            <Link href={`/corrections/from/${page.next_cursor}`}>Older corrections</Link>
          ) : null}
        </nav>
      </section>
    </>
  );
}
