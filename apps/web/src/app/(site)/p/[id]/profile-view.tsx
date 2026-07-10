import Link from "next/link";
import { notFound } from "next/navigation";

import { ApiError, politicianProfile, politicianRecords } from "@/lib/api";
import { formatDate } from "@/lib/format";
import { RecordTable } from "@/components/RecordRows";

export async function fetchProfileOr404(id: string) {
  try {
    return await politicianProfile(id);
  } catch (error) {
    if (error instanceof ApiError && (error.status === 404 || error.status === 400)) {
      notFound();
    }
    throw error;
  }
}

// Shared by /p/[id] (first page) and /p/[id]/from/[cursor] (later pages).
// Pagination lives in the PATH, not a query string: records are immutable
// and ULID-ordered, so each timeline page is a permanent, CDN-cacheable URL.
export async function ProfileView({ id, cursor }: { id: string; cursor?: string }) {
  const profile = await fetchProfileOr404(id);
  let timeline;
  try {
    timeline = await politicianRecords(id, cursor === undefined ? {} : { cursor });
  } catch (error) {
    if (error instanceof ApiError && error.status === 400) {
      notFound(); // malformed cursor in the URL
    }
    throw error;
  }

  return (
    <>
      <section className="profile-head">
        <h1>{profile.canonical_name}</h1>
        {profile.mandates.length > 0 ? (
          <ul className="mandates">
            {profile.mandates.map((mandate) => (
              <li key={mandate.id}>
                <span className="role">
                  {mandate.role}, {mandate.body}
                </span>
                {mandate.district ? ` · ${mandate.district}` : null}
                {mandate.party ? ` · ${mandate.party}` : null}
                {" · since "}
                {formatDate(mandate.start_date)}
                {mandate.end_date ? ` until ${formatDate(mandate.end_date)}` : null}
              </li>
            ))}
          </ul>
        ) : (
          <p className="muted">No mandates on file.</p>
        )}
        <p className="muted">
          {profile.records.count} disclosure record
          {profile.records.count === 1 ? "" : "s"} on file
          {profile.records.first_event_date && profile.records.last_event_date
            ? ` · ${formatDate(profile.records.first_event_date)} to ${formatDate(
                profile.records.last_event_date,
              )}`
            : null}
          {profile.wikidata_qid ? (
            <>
              {" · "}
              <a
                href={`https://www.wikidata.org/wiki/${encodeURIComponent(profile.wikidata_qid)}`}
                rel="noopener noreferrer"
              >
                {profile.wikidata_qid}
              </a>
            </>
          ) : null}
        </p>
      </section>

      <section aria-label="Disclosure timeline">
        <h2>Disclosure timeline</h2>
        <div className="table-scroll">
          <RecordTable
            records={timeline.items}
            caption={`Disclosure records of ${profile.canonical_name}`}
          />
        </div>
        <nav className="pager" aria-label="Timeline pages">
          {cursor !== undefined ? <Link href={`/p/${profile.id}`}>Newest page</Link> : null}
          {timeline.next_cursor !== null && timeline.next_cursor !== undefined ? (
            <Link href={`/p/${profile.id}/from/${timeline.next_cursor}`}>Older records</Link>
          ) : null}
        </nav>
      </section>
    </>
  );
}
