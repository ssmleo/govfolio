# br politician-identity-collision remediation — plan (NOT executed)

Status: PROPOSED — planning only, per explicit instruction. No code was written, no
migration was run, no database was touched except read-only `SELECT`s used to verify
every claim in this document against the live shared dev DB. Ready for a rust-builder +
auditor pair to execute, staged-then-promoted (goal 022 discipline), with the elevated
review gate in "Execution authority" below.

## 1. The defect (confirmed, live, as of 2026-07-07)

Two different real Brazilian people, both named `JULIO CESAR DOS SANTOS`, both 2018
`DEPUTADO FEDERAL` candidates in Bahia (`BA`), currently share one `politician` row:

- `politician.id = 01KWXE3M4J18YNCD5R1V7NTGQ3`, `canonical_name = 'JULIO CESAR DOS SANTOS'`
- one `politician_alias` row (same text, `lang = NULL`)
- one `mandate` row: `01KWXE3M4JBKKBNCD14EN1CJ0P` (Câmara dos Deputados, DEPUTADO FEDERAL,
  `party = NULL`, `district = 'BA'`, `start_date = 2018-01-01`, `end_date = NULL`)
- two `filing` rows, both `regime_id = 0BRAREG0000000000000000001` (Câmara), both
  `filed_date = 2018-10-07`, discovered ~0.7s apart in the same backfill run:

  | filing.id | external_id | CPF (stg_br) | título eleitoral | disclosure_records |
  |---|---|---|---|---|
  | `01KWXEDGZQ8K0E9ZA75PB5C0ZS` | `2018:50000608317` | `67701124500` | `066773530590` | 1 (bank deposit, `#NULO#`, 10000.00 BRL) |
  | `01KWXEDHNW2T2YZFQW2QEVE5EM` | `2018:50000604277` | `80673872653` | `088320410230` | 4 (land, cash, car, savings) |

Verified independently twice already this session (producer + auditor, both against raw
Bronze bytes). Re-verified a third time while writing this plan, directly against
`stg_br.nr_cpf_candidato`/`nr_titulo_eleitoral_candidato` (see §2 for why that column is
trustworthy) — same two CPFs, same split.

Root cause: a leftover of the documented "first buggy nationwide seed pass" (pre-
`identity_collision_counts` fix) whose 89-pair cleanup was not fully exhaustive. Not
caused by this session's SENADOR-widen or any other recent work — discovered by it.

## 2. Question 1 — is "two people, two rows" the right end state? (yes; confirmed, no reconciliation needed)

Framing confirmed: the correct end state is each real person with their own
`politician` row and their own correctly-attributed filing/records. The one thing that
framing could be wrong about is if either person *already* has another correctly-
attributed candidacy elsewhere under a different `politician_id` — in which case the fix
would need to attach this filing to that existing row instead of minting a new one.

Checked this directly, not assumed. `stg_br.nr_cpf_candidato`/`nr_titulo_eleitoral_candidato`
are populated for **100% of real (non-fixture) rows** (`80420/80420`, `null_cpf = 0` —
confirmed live) because they are gated on `ctx.pool.is_some()`, which is true for every
real backfill run this session performed. That makes a full-population CPF-collision
sweep possible right now, cheaply, with SQL alone:

```sql
select p.id as politician_id, p.canonical_name,
       count(distinct s.nr_cpf_candidato) as distinct_cpfs,
       array_agg(distinct s.nr_cpf_candidato) as cpfs
from politician p
join filing f on f.politician_id = p.id
join raw_document rd on rd.id = f.raw_document_id
join stg_br s on s.raw_document_id = rd.id
where s.nr_cpf_candidato is not null
group by p.id, p.canonical_name
having count(distinct s.nr_cpf_candidato) > 1;
```

Run live during this planning pass: **exactly one row comes back** —
`01KWXE3M4J18YNCD5R1V7NTGQ3`, the two CPFs above. This is a stronger check than the
session's earlier 16-case same-year-multi-filing sample (which only catches multiple
filings landing on one politician *in the same year*): this sweep is exhaustive across
the whole `br` dataset, every year, every body, and confirms **this is the only live
collision in the database today.**

It also directly answers the reconciliation question: neither CPF (`80673872653` nor
`67701124500`) appears in `stg_br` under any *other* `raw_document_id` — each of these
two real people has exactly one candidacy, exactly one filing, in the whole dataset.
There is no other existing, correctly-attributed row to reconcile with. **The fix is a
clean split, not a merge-with-a-third-row.**

Recommendation: re-run this exact query as step 0 of execution (§5) to reconfirm nothing
changed between planning and execution, and adopt it going forward (§7) as a cheap
standing detection net independent of the harder prevention work.

## 3. Question 2 — supersede-never-update vs. a foreign-key repoint (repoint; not a supersede)

Checked how supersession actually works in this schema before assuming either model applies.

`filing.supersedes_filing_id` and `disclosure_record.supersedes_record_id` (migration
`0001_core.sql`) model **evolving disclosed facts** — an amendment restates an amount, a
reviewer edit corrects an extracted value. `crates/pipeline/src/promote.rs`'s `supersede()`
is the one place that inserts a superseding record, and it is instructive by exactly what
it refuses to do:

> "Corrected facts become a new row... Identity (filing/politician/regime) is **pinned
> from the original row** — reviewer-supplied identity fields are ignored, **corrections
> cannot rebind a record**." (`promote.rs`, `Verdict::Edit` doc comment; enforced in code:
> `bound.politician_id = original.politician_id.parse()...`)

This is the schema's own designed answer to question 2: the supersede path is *built to
be incapable* of repointing `politician_id`, on purpose, because in the case it exists
for (a reviewer correcting a disclosed value), the politician attribution was never in
question — only the value was. Here it's the reverse: the disclosed facts (the land, the
cash, the car, the bank deposit, the exact BRL amounts) are **not wrong** — TSE's own
data says what it says, verbatim, for each of the two real CPFs. What's wrong is which
`politician_id` the pipeline's roster resolution attached the filing to. That is a
resolution bug, not an evolving fact, and invariant 1 does not require versioning a bug
in an FK the same way it requires versioning a value a human reviewer disputes.

**Recommendation: this is a direct, targeted repoint of the `politician_id` foreign key
on the affected `filing` and `disclosure_record` rows — not a supersede, not a new
`'corrected'` record, and not a run through `promote.rs`'s reviewer path (which
structurally cannot do this and should not be made to).** `verification_state` on the
moved records stays `'unverified'` — the disclosed *content* was never reviewed either
way; only its attribution changes.

## 4. Question 3 — mechanical fix: pipeline re-run, or direct SQL? (direct, reviewed, outside the Runner)

Confirmed the auditor's finding by reading `crates/pipeline/src/run.rs` directly.
`process_document`'s publish claim key is `format!("{code}:publish:{sha256}")` — content
+ adapter code, not `regime_id`/`body`/entity. Both filings' publish claims are recorded
`succeeded` in `pipeline_run` right now (`br:publish:83e06691...` and
`br:publish:c7b7ce30...`, both `status = 'succeeded'`). A plain re-run hits
`Claim::Replay` at the publish stage and returns without ever calling
`publish_document`/`roster::resolve_politician` again — confirmed, not assumed.

Even setting that aside: forcing a re-run by invalidating those two claim rows would not
fix anything on its own. `roster::resolve_politician` matches on `(alias, district,
body)` only — it has no CPF signal — so re-resolving today would find the *same* single
existing politician row (nothing yet distinguishes the two people to the resolver) and
reproduce the exact same bug. Building a CPF-aware resolver is the "broader mechanism"
(§7), out of scope for a one-off fix.

**Recommendation: the fix does not route through `Runner`/`publish_filing` at all.** It
is a direct, transaction-wrapped data correction, executed by a small reviewed one-off
Rust program — the same established convention this repo already uses for real,
targeted DB writes outside the migration system (`crates/worker/src/bin/backfill-real-br.rs`,
`seed-br-candidates.rs`, `local_br.rs`): dry-run by default, explicit flag to write,
one transaction, before/after counts printed and checked.

**Not a `crates/core/migrations/*.sql` migration, and `scripts/check-migration-safety.sh`
does not apply.** That gate `grep`s for destructive DDL (`DROP`/`TRUNCATE`/`ALTER...DROP`)
in the migrations directory — it exists to protect schema changes. This fix touches zero
schema (no `CREATE`/`ALTER`/`DROP`) — it is pure DML (`INSERT`/`UPDATE`) against existing
tables, which is exactly the shape of every other one-off `worker::bin` write this
session already performed and had independently audited. Putting a one-off data
correction into the permanent schema-migration history would also be the wrong
container for it — migrations version *schema*, not one dev database's one bug.

**Do not touch the two `pipeline_run` claim rows.** Leaving them `succeeded` is
correct and desired: it guarantees a future bulk `br` re-run continues to replay past
these two documents rather than re-touching (or re-breaking) a row this plan just fixed
by hand.

## 5. Question 4 — which person keeps the existing `politician_id`? (whichever minimizes touched rows; not a "which is the real one" question)

`politician.id` is an opaque internal ULID with no public meaning attached to
precedence — nothing in the schema or the API surfaces "this politician's ID is older/
more original than that one's." Retiring *both* ids and minting two fresh ones would
touch strictly more rows (both people's `filing`/`disclosure_record` FKs move instead of
one) for zero correctness benefit, and would need to explain "why did the untouched
person's ID also change" in the audit trail for no reason. That's the opposite of the
surgical, minimal-diff standard this session already holds itself to.

**Recommendation: keep the existing `politician_id` for the person with more attached
Gold rows, and mint a new one only for the smaller side.** Concretely: CPF `80673872653`
(4 disclosure_records: land/cash/car/savings, filing `01KWXEDHNW2T2YZFQW2QEVE5EM`) keeps
`01KWXE3M4J18YNCD5R1V7NTGQ3` untouched — zero rows move for this person, zero
regression risk. CPF `67701124500` (1 disclosure_record, filing
`01KWXEDGZQ8K0E9ZA75PB5C0ZS`) gets a freshly minted `politician_id`. This is a pure
row-count tie-breaker, not a statement that one person is "more legitimate" than the
other — both are equally real, equally correctly attributed once this runs.

## 6. Question 5 — blast radius: every table/row that must move together

Confirmed exhaustively by querying `information_schema.columns` for every table with a
`politician_id` column (live, not from memory): **`disclosure_record`, `filing`,
`mandate`, `politician_alias`** — exactly four, matching `crates/core/migrations/0001_core.sql`.
Nothing else in the schema references `politician.id`.

For this specific case:

- **`mandate`**: exactly one row for the shared politician (`01KWXE3M4JBKKBNCD14EN1CJ0P`).
  `party` is `NULL` on it — no per-person data is at risk of being silently attributed to
  the wrong person here; the new mandate row is a byte-identical copy (new `id`/
  `politician_id` only).
- **`politician_alias`**: exactly one row, same story — a byte-identical copy under the
  new `politician_id`.
- **`filing`**: exactly one of the two filings moves (`01KWXEDGZQ8K0E9ZA75PB5C0ZS`).
- **`disclosure_record`**: exactly one row moves (`01KWXEDGZR51RXG2QM5SHGEYW9`, the bank
  deposit) — it denormalizes `politician_id` (design's own "hot path" denorm) and must
  move in the SAME transaction as its parent `filing` row, never out of sync.
- **`review_task`**: checked directly — **zero** open or resolved tasks reference either
  filing's `external_id` or the shared `politician_id` (`unresolved_filer` never fired
  here, because resolution *succeeded*, onto the wrong row — that's the whole bug).
  Nothing to move.
- **`outbox_event`**: **5 rows** (1+4) carry the shared `politician_id` in their JSONB
  `payload`, all already `dispatched_at`-stamped (backfill mode) — confirmed live. These
  are inert historical dispatch records, not a live query surface (nothing re-reads
  `outbox_event.payload` for correctness; the matcher only ever reads undispatched rows).
  **Recommendation: leave them as-is.** Repointing history that's already "happened" (in
  the dispatch-suppression sense) adds risk for no consumer-visible benefit; note the
  staleness in the journal write-back instead of writing to this table.
- **`pipeline_run`**: the two `parse`/`publish` claim rows — deliberately **left
  untouched** (§4).
- **No materialized views, no cache tables, nothing web-side found.** `apps/web`'s SSR
  profile pages read live from `politician`/`mandate`/`disclosure_record` per request
  (no snapshot table feeding them was found); if any CDN/ISR caching exists in front of
  `/p/[id]` in a deployed environment, purge/revalidate both politician ids as a
  post-deploy hygiene step — not applicable to the shared local dev DB this plan targets.

## 7. Question 6 — verification plan (independent, re-runnable, idempotent)

An independent auditor should confirm, against the live DB, **after** execution:

1. **Row-count sweep (the whole point of the fix).** Re-run the §2 CPF-collision query —
   must now return **zero rows**.
2. **Each person's profile is complete and self-consistent:**
   ```sql
   -- old id: must show canonical_name + 1 mandate + 4 disclosure_records (land/cash/car/savings)
   select * from politician where id = '01KWXE3M4J18YNCD5R1V7NTGQ3';
   select * from mandate where politician_id = '01KWXE3M4J18YNCD5R1V7NTGQ3';
   select * from disclosure_record where politician_id = '01KWXE3M4J18YNCD5R1V7NTGQ3';
   -- new id: must show canonical_name + 1 mandate + 1 disclosure_record (bank deposit)
   select * from politician where id = '<NEW_ID>';
   select * from mandate where politician_id = '<NEW_ID>';
   select * from disclosure_record where politician_id = '<NEW_ID>';
   ```
   Assert: the 4-record set's `asset_description_raw`/`value_low`/`currency` values are
   byte-identical, in the same rows (same `id`s), to what existed before the fix — only
   `politician_id` (on the moved side) and the recomputed `fingerprint` (moved side only)
   changed. Nothing about the untouched (larger) side's rows changed at all — diff their
   `id`/`fingerprint`/every column against a pre-change snapshot (§8) and expect zero
   differences.
3. **CPF re-check against raw Bronze**, exactly the project's own established method:
   pull the two documents by sha256 (`83e06691b0...`/`c7b7ce30...`) from
   `target/bronze-backfill-real-br` (see §8's note on why not `storage_uri`), re-derive
   `NR_CPF_CANDIDATO` for each, and confirm CPF `80673872653` → the 4-record politician,
   CPF `67701124500` → the new 1-record politician.
4. **Fingerprint self-consistency.** For the moved record, recompute
   `worker::backfill::candidate_fingerprints("br", baseline, candidates)` twice: once
   with `baseline.politician_id` = the OLD shared id (must reproduce the **pre-change**
   stored fingerprint exactly — proves the re-derivation pipeline is exact) and once with
   the NEW id (must equal the **post-change** stored fingerprint exactly). Both checks
   must pass; if either fails, the fix's fingerprint step was wrong and must be redone,
   not patched further.
5. **Nothing else moved.** Row counts for `review_task`, `outbox_event`, `pipeline_run`,
   and every other `br` table are identical before/after except the specific
   insert/update rows enumerated in §8 — assert via total-row-count diff per table, the
   same discipline this session already used for every backfill pass.
6. **Idempotency / safe re-verification.** Re-running the §2 sweep and the profile
   queries above a second time must be side-effect-free (they are pure `SELECT`s) and
   must return the same answers. The fix script itself (§8) must be safe to invoke twice:
   its second invocation must detect the split is already done (e.g. the CPF sweep
   already returns zero rows for this politician) and no-op rather than erroring or
   double-inserting — mirroring the idempotency discipline every other `br` write this
   session already proved.
7. **Full regression gates**, unchanged from every other `br` pass this session:
   `cargo build -p worker -p br`, `cargo fmt --check`, `cargo clippy --all-targets -- -D
   warnings`, `cargo run -p pipeline --bin conformance -- br` (3/3, must stay green — this
   fix touches no adapter/conformance code), `cargo test -p pipeline --test role_evals`
   (11/11).

## 8. Execution plan (concrete, ordered, transaction-wrapped)

Build one new one-off binary, `crates/worker/src/bin/fix-br-julio-cesar-santos-ba-2018.rs`
(narrowly named and narrowly scoped to this exact case — do not generalize it into a
"fix any collision" tool yet; if a second case surfaces before the broader mechanism
lands, factor the shared logic out then, not speculatively now). Follow the existing
`backfill-real-br.rs` shape: dry-run by default, `--execute` to write, one DB
transaction, structured before/after report.

**Step 0 — reconfirm scope (read-only).** Re-run the §2 CPF-collision sweep. Must return
exactly the one row already found. If it returns anything else (more rows, zero rows,
different CPFs), STOP — the premise this plan was built on has changed; do not proceed
on a stale assumption (this is a halt, not a guess, per project standard).

**Step 1 — pre-change snapshot (mandatory, mirrors the automation policy's "mandatory
pre-apply snapshot" requirement for prod migrations, applied here by analogy given the
recent invariant-2 incident).** Export, verbatim, every row this plan touches or reads
as a baseline:
```sql
\copy (select * from politician where id = '01KWXE3M4J18YNCD5R1V7NTGQ3') to 'snapshot_politician.csv' csv header
\copy (select * from politician_alias where politician_id = '01KWXE3M4J18YNCD5R1V7NTGQ3') to 'snapshot_alias.csv' csv header
\copy (select * from mandate where politician_id = '01KWXE3M4J18YNCD5R1V7NTGQ3') to 'snapshot_mandate.csv' csv header
\copy (select * from filing where politician_id = '01KWXE3M4J18YNCD5R1V7NTGQ3') to 'snapshot_filing.csv' csv header
\copy (select * from disclosure_record where politician_id = '01KWXE3M4J18YNCD5R1V7NTGQ3') to 'snapshot_records.csv' csv header
```
(or a full `pg_dump` of the local dev DB — cheap, and simplest to get exactly right.)

**Step 2 — re-derive candidates from Bronze (offline, read-only), self-check first.**
Using the recovered Bronze bytes — **read via `BronzeStore::get(RawDocRef{sha256})`,
keyed purely by sha256, NOT via `raw_document.storage_uri`**, which is known-stale for
these two rows (still points at the deleted `%TEMP%\govfolio-backfill-real-br-15648\...`
path per the INCIDENT RESOLVED journal entry; the bytes live under
`target/bronze-backfill-real-br` post-recovery) — run `BrAdapter::parse()` +
`BrAdapter::normalize()` for `raw_document_id = 01KWXEDGZERRECSZM6W1BB60BH`
(sha256 `83e06691b033742bd23e9d347a734603a4082ed5df20285e5938a0c220dc0b37`) to get its one
`GoldCandidate` back. Build a `worker::backfill::FilingBaseline { filing_id:
"01KWXEDGZQ8K0E9ZA75PB5C0ZS", politician_id: "01KWXE3M4J18YNCD5R1V7NTGQ3" (the OLD id),
regime_id: "0BRAREG0000000000000000001", .. }` and call `candidate_fingerprints("br",
&baseline, &candidates)`. **Assert the single result equals the currently stored
`disclosure_record.fingerprint` (`31924765f131c9bb0cdd2191a9f9899f44e0454042742590b1f23f2c1ce70773`)
exactly.** If it does not match, STOP — do not guess at the discrepancy; the
re-derivation is not yet proven exact and nothing downstream may be trusted.

**Step 3 — compute the corrected fingerprint.** Rebuild the same `FilingBaseline` with
`politician_id` = the freshly minted new id (generate via `ulid::Ulid::new()` at run
time — do not hardcode one in code or docs), call `candidate_fingerprints` again. This
is the value Step 5 writes.

**Step 4 — mint the new identity rows (one transaction with steps 5–6):**
```sql
insert into politician (id, canonical_name, wikidata_qid, details)
values ($new_politician_id, 'JULIO CESAR DOS SANTOS', null, '{}');

insert into politician_alias (politician_id, alias, lang)
values ($new_politician_id, 'JULIO CESAR DOS SANTOS', null);

insert into mandate (id, politician_id, jurisdiction_id, body, role, party, district, start_date, end_date)
values ($new_mandate_id, $new_politician_id, 'br', 'Câmara dos Deputados', 'DEPUTADO FEDERAL', null, 'BA', '2018-01-01', null);
```

**Step 5 — repoint the filing (same transaction):**
```sql
update filing set politician_id = $new_politician_id
where id = '01KWXEDGZQ8K0E9ZA75PB5C0ZS' and politician_id = '01KWXE3M4J18YNCD5R1V7NTGQ3';
-- assert rows_affected == 1
```

**Step 6 — repoint the record and its fingerprint (same transaction):**
```sql
update disclosure_record
set politician_id = $new_politician_id, fingerprint = $new_fingerprint_from_step_3
where id = '01KWXEDGZR51RXG2QM5SHGEYW9'
  and politician_id = '01KWXE3M4J18YNCD5R1V7NTGQ3'
  and fingerprint = '31924765f131c9bb0cdd2191a9f9899f44e0454042742590b1f23f2c1ce70773';
-- assert rows_affected == 1 (the old-value guard in WHERE makes this safe to re-run:
-- a second invocation finds 0 rows matching the OLD politician_id/fingerprint and can
-- detect "already applied" rather than silently re-writing)
```

**Step 7 — commit, then re-run §2's sweep and §7's full verification list inside the
same script invocation before reporting success.** Do not commit-and-hope; the
verification queries are cheap and belong in the same run.

**Step 8 — write-back.** A `docs/regimes/br/AUTHORITY.md` Quirks log entry marking this
specific finding RESOLVED (old finding is already logged there — 2026-07-07 SENADOR
historical re-run entry), pointing at the commit; a `agents/JOURNAL.md` entry with the
before/after ids/counts, mirroring every other fix this session recorded; independent
auditor re-verification per §7, PASS/BOUNCE per goal 022 discipline.

## 9. Question 7 — fix now, or wait for the broader CPF-aware mechanism?

**Recommendation: fix this specific case now, as the scoped one-off above; track the
general CPF/voter-title-aware cross-time/cross-body identity mechanism as separate,
larger future work — do not block on it.**

Reasoning:
- The broader mechanism is a real, multi-axis project (seed-time same-pass collisions —
  already partly fixed; cross-year linking; cross-body linking; and now, this
  within-body/within-year leftover-merge axis) spanning several future goals, not a
  quick patch. Holding a known, fully-diagnosed, currently-live wrong attribution
  hostage to that timeline serves no one — the two real people involved deserve their
  correct profiles now, not after an open-ended architecture project lands.
- This case is unusually low-risk to fix in isolation: §2's exhaustive sweep proves it
  is the *only* live case, so there's no reconciliation complexity, no multi-way merge,
  and the touched-row footprint is tiny (3 inserts, 2 updates, both guarded and
  idempotent).
- Fixing this one case does not complicate or foreclose the broader mechanism later —
  the broader work is about *preventing* future collisions at seed/resolve time; it will
  always need some one-off reconciliation story for whatever it finds in already-published
  history, and this plan (and its §2 sweep) is a reasonable template for that.
- The one piece of this that *should* happen alongside — cheaply, now — is adopting §2's
  sweep query as a standing, mechanical detection net (e.g. run it as part of any future
  `br` epoch milestone, or wire it into the auditor's routine checks) so this defect class
  cannot silently reappear at nationwide scale before the harder prevention mechanism
  exists. That is a report/alert-only addition (no schema, no behavior change) and is
  much cheaper than the full identity-resolution redesign — worth doing now even though
  the full redesign should not block on it either.

## 10. Overall assessment

**Safe to execute as a normal follow-up goal, not a `blocked:founder` item** — this is a
dev-only, schema-untouched, small-blast-radius, fully-diagnosed, mechanically verifiable
data correction with no open ambiguity left (every one of the 7 questions above was
resolved by reading code/schema or querying the live DB directly, not assumed).

One elevated, self-clearing procedural gate is warranted given (a) this is the first
politician-identity-split this codebase has ever performed, so there is no prior audited
precedent for this exact write shape, and (b) the recent invariant-2 incident showed what
an under-verified destructive/mutating action on this same class of data costs: **the
one-off script itself should be code-reviewed by an independent auditor BEFORE its first
`--execute` run against the shared dev DB** (review-before-run), rather than the
after-the-fact audit pattern most other one-off `br` bins in this session used. Once that
review passes, execution proceeds under the standing autonomy policy — no founder wait.
Recommended tag if filed as a goal: `ready:auditor-pre-review` (not `blocked:founder`).
