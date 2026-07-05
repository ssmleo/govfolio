// E2E seeding for the reviewer flows. Review tasks (and the records they
// target) are PIPELINE data — seeding rows here mirrors what the pipeline's
// publish stage does, exactly like the API contract tests' `seed_task`;
// adjudication itself still only ever happens through the resolve endpoint.
//
// Why clone records instead of targeting seeded ones: promote's `transition`
// fails closed unless the record is still `unverified`, so repeated e2e runs
// would exhaust the fixture records. Each run clones a real pipeline-produced
// row (same filing/politician/regime/details) into a fresh unverified record
// with its own fingerprint, and opens a task against it.
import { Client } from "pg";

const DATABASE_URL =
  process.env.DATABASE_URL ?? "postgres://postgres:postgres@localhost:5433/govfolio";

const CROCKFORD = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/** Minimal ULID: 10-char Crockford timestamp + 16 random chars (26 total). */
export function ulid(timestamp: number = Date.now()): string {
  let time = "";
  let remaining = timestamp;
  for (let i = 0; i < 10; i += 1) {
    time = CROCKFORD.charAt(remaining % 32) + time;
    remaining = Math.floor(remaining / 32);
  }
  let random = "";
  for (let i = 0; i < 16; i += 1) {
    random += CROCKFORD.charAt(Math.floor(Math.random() * 32));
  }
  return time + random;
}

export interface SeededReviewCase {
  taskId: string;
  recordId: string;
}

/**
 * Clones one pipeline-produced `unverified` transaction record into a fresh
 * row and opens an open review task against it, at a priority high enough to
 * top the queue. Returns both ids.
 */
export async function seedReviewCase(options?: {
  reason?: string;
  priority?: number;
}): Promise<SeededReviewCase> {
  const reason = options?.reason ?? "ptr_amendment_unlinked";
  const priority = options?.priority ?? 9.5;
  const client = new Client({ connectionString: DATABASE_URL });
  await client.connect();
  try {
    const source = await client.query<{ id: string }>(
      `select id from disclosure_record
       where record_type = 'transaction'
         and supersedes_record_id is null
         and verification_state = 'unverified'
       order by id
       limit 1`,
    );
    const sourceRow = source.rows[0];
    if (!sourceRow) {
      throw new Error(
        "no unverified transaction record to clone — seed the DB first: cargo run -p worker --bin local",
      );
    }
    const recordId = ulid();
    await client.query(
      `insert into disclosure_record
         (id, filing_id, politician_id, regime_id, instrument_id,
          asset_description_raw, record_type, asset_class, side,
          transaction_date, as_of_date, notified_date,
          value_low, value_high, currency, owner, verification_state,
          extraction_confidence, extracted_by, fingerprint,
          supersedes_record_id, details)
       select $1, filing_id, politician_id, regime_id, instrument_id,
          asset_description_raw, record_type, asset_class, side,
          transaction_date, as_of_date, notified_date,
          value_low, value_high, currency, owner, 'unverified',
          extraction_confidence, extracted_by, $2,
          null, details
       from disclosure_record where id = $3`,
      [recordId, `e2e:${recordId}`, sourceRow.id],
    );
    const taskId = ulid();
    await client.query(
      `insert into review_task (id, target_kind, target_id, reason, priority_score)
       values ($1, 'disclosure_record', $2, $3, $4)`,
      [taskId, recordId, reason, priority],
    );
    return { taskId, recordId };
  } finally {
    await client.end();
  }
}
