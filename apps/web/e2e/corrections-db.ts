// E2E seeding for the public corrections log. A correction is PIPELINE data
// (promote's supersede inserts a new `corrected` row that supersedes the
// original — invariant 1); seeding the pair directly here mirrors what promote
// writes, exactly like reviewer-db.ts clones records for the reviewer flows.
//
// Both rows are cloned from a real, currently-visible pipeline record so their
// filing/politician/regime foreign keys are valid and the pair passes the
// free-tier freshness gate (the source is already published). Fixed, minimal
// ULIDs + ON CONFLICT DO NOTHING make the seed idempotent (corrected e2e rows
// never accumulate) and sort the correction to the FRONT of the ascending
// listing, so it is always on the first /corrections page.
import { Client } from "pg";

const DATABASE_URL =
  process.env.DATABASE_URL ?? "postgres://postgres:postgres@localhost:5433/govfolio";

// 26-char Crockford ULIDs with an all-zero timestamp → the smallest possible
// ids, so this pair leads the ascending record listing regardless of how many
// real corrections exist.
const ORIGINAL_ID = "00000000000000000000000RG1";
const CORRECTED_ID = "00000000000000000000000CR1";

export interface SeededCorrection {
  originalId: string;
  correctedId: string;
}

// Shared identity clone with an overridden value band, verification state,
// fingerprint, id and supersession pointer.
const CLONE_SQL = `insert into disclosure_record
    (id, filing_id, politician_id, regime_id, instrument_id,
     asset_description_raw, record_type, asset_class, side,
     transaction_date, as_of_date, notified_date,
     value_low, value_high, currency, owner, verification_state,
     extraction_confidence, extracted_by, fingerprint,
     supersedes_record_id, details)
  select $1, filing_id, politician_id, regime_id, instrument_id,
     asset_description_raw, record_type, asset_class, side,
     transaction_date, as_of_date, notified_date,
     $2, $3, currency, owner, $4,
     extraction_confidence, extracted_by, $5,
     $6, details
  from disclosure_record where id = $7
  on conflict (id) do nothing`;

/**
 * Seeds one correction pair (original + the `corrected` row superseding it,
 * with a widened declared band so a before -> after change is visible) and
 * returns both ids. Idempotent across runs.
 */
export async function seedCorrection(): Promise<SeededCorrection> {
  const client = new Client({ connectionString: DATABASE_URL });
  await client.connect();
  try {
    const source = await client.query<{ id: string }>(
      `select id from disclosure_record
       where record_type = 'transaction' and supersedes_record_id is null
       order by id
       limit 1`,
    );
    const src = source.rows[0];
    if (!src) {
      throw new Error(
        "no source record to clone — seed the DB first: cargo run -p worker --bin local",
      );
    }
    // Original (the earlier declared band, later corrected).
    await client.query(CLONE_SQL, [
      ORIGINAL_ID,
      "1001.00",
      "15000.00",
      "unverified",
      `e2e-corr-orig-${ORIGINAL_ID}`,
      null,
      src.id,
    ]);
    // Correction: supersedes the original, band widened upward.
    await client.query(CLONE_SQL, [
      CORRECTED_ID,
      "15001.00",
      "50000.00",
      "corrected",
      `e2e-corr-corr-${CORRECTED_ID}`,
      ORIGINAL_ID,
      src.id,
    ]);
    return { originalId: ORIGINAL_ID, correctedId: CORRECTED_ID };
  } finally {
    await client.end();
  }
}
