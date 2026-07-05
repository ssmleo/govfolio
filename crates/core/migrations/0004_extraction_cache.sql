-- 0004_extraction_cache: LLM extraction cache (design §5.3 "cache by SHA",
-- goal 021). Expand-only.
--
-- One extraction per DOCUMENT VERSION: key = (document_sha256, extractor_tag,
-- model_id) — re-extraction happens only on an extractor/model version bump.
-- `rows` is the full Silver-row array ([{payload, confidence}], the pipeline
-- StagingRow wrapper shape); `provenance` records how the entry was produced
-- (live model call + cross-check verdict, or mechanical ground-truth priming
-- in conformance fixtures). Writes are ON CONFLICT DO NOTHING (invariant 4).

create table extraction_cache (
  document_sha256 text        not null check (document_sha256 ~ '^[0-9a-f]{64}$'),
  extractor_tag   text        not null,
  model_id        text        not null,
  rows            jsonb       not null,
  provenance      jsonb       not null default '{}'::jsonb,
  created_at      timestamptz not null default now(),
  primary key (document_sha256, extractor_tag, model_id)
);
