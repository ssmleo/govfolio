// Server-side typed client over the GENERATED contract (@govfolio/contracts).
// Every request/response type is derived from `paths` — no hand-rolled
// response shapes (language boundary: the OpenAPI contract is the only door).
//
// ETag leverage (design §6.4 read-scaling): the API sets a strong ETag on
// every GET. We keep a bounded per-process cache of etag+body and send
// If-None-Match; a 304 reuses the cached body instead of re-downloading.

import type { components, paths } from "@govfolio/contracts";

type Schemas = components["schemas"];
export type Politician = Schemas["Politician"];
export type PoliticianProfile = Schemas["PoliticianProfile"];
export type PoliticianPage = Schemas["PoliticianPage"];
export type DisclosureRecord = Schemas["DisclosureRecord"];
export type RecordDetail = Schemas["RecordDetail"];
export type RecordPage = Schemas["RecordPage"];
export type Provenance = Schemas["Provenance"];
export type Jurisdiction = Schemas["Jurisdiction"];
export type Regime = Schemas["Regime"];
export type SearchResults = Schemas["SearchResults"];
export type ValueInterval = Schemas["ValueInterval"];
export type VerificationState = Schemas["VerificationState"];
export type Mandate = Schemas["Mandate"];
export type RecordType = Schemas["RecordType"];
export type AssetClass = Schemas["AssetClass"];
export type Side = Schemas["Side"];
export type Owner = Schemas["Owner"];
export type Currency = Schemas["Currency"];
export type ReviewTask = Schemas["ReviewTask"];
export type ReviewQueueItem = Schemas["ReviewQueueItem"];
export type ReviewQueuePage = Schemas["ReviewQueuePage"];
export type ReviewTargetSummary = Schemas["ReviewTargetSummary"];
export type ReviewTaskDetail = Schemas["ReviewTaskDetail"];
export type ReviewAuditEntry = Schemas["ReviewAuditEntry"];
export type ExtractionContext = Schemas["ExtractionContext"];
export type ResolveResponse = Schemas["ResolveResponse"];
type ResolveRequest = Schemas["ResolveRequest"];
type ErrorBody = Schemas["ErrorBody"];

/**
 * Corrected facts for an edit, in the `GoldCandidate` wire shape — derived
 * from the GENERATED `DisclosureRecord` schema (the wire shapes mirror each
 * other field-for-field). Identity fields (`filing_id`, `politician_id`,
 * `regime_id`) and `fingerprint` are omitted on purpose: promote pins
 * identity from the original row and computes the fingerprint (invariant 4).
 * `value` bounds stay DECIMAL STRINGS end to end (invariant 7).
 */
export type CorrectedPayload = Pick<
  DisclosureRecord,
  | "instrument_id"
  | "asset_description_raw"
  | "record_type"
  | "asset_class"
  | "side"
  | "transaction_date"
  | "as_of_date"
  | "notified_date"
  | "value"
  | "owner"
  | "extraction_confidence"
  | "extracted_by"
  | "details"
>;

/**
 * The resolve request as the UI sends it: the generated `ResolveRequest`
 * with its opaque `corrected` arm (schemars renders `serde_json::Value` as an
 * untyped object) narrowed to the wire shape promote actually deserializes.
 */
export interface ResolveInput extends Omit<ResolveRequest, "corrected" | "verdict"> {
  verdict: "confirm" | "edit" | "reject";
  corrected?: CorrectedPayload | null;
}

/** What a resolve attempt came to, projected for the reviewer UI. */
export type ResolveActionResult =
  | { kind: "applied"; recordId: string; supersedingRecordId: string | null }
  | { kind: "conflict"; message: string }
  | { kind: "error"; code: string; message: string };

/** Paths that expose a GET operation. */
type GetPath = {
  [P in keyof paths]: paths[P] extends { get: { responses: unknown } } ? P : never;
}[keyof paths];

/** The 200 application/json body of a GET operation. */
type GetOk<P extends GetPath> = paths[P] extends {
  get: { responses: { 200: { content: { "application/json": infer B } } } };
}
  ? B
  : never;

/** The query parameters of a GET operation (never → no query accepted). */
type GetQuery<P extends GetPath> = paths[P] extends {
  get: { parameters: { query?: infer Q } };
}
  ? NonNullable<Q>
  : never;

export class ApiError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(`${code}: ${message}`);
    this.name = "ApiError";
    this.status = status;
    this.code = code;
  }
}

export function apiBaseUrl(): string {
  return process.env.GOVFOLIO_API_URL ?? "http://localhost:8080";
}

interface EtagEntry {
  etag: string;
  body: string;
}

const ETAG_CACHE_MAX = 500;
const etagCache = new Map<string, EtagEntry>();

/**
 * Admin bootstrap header for the review surface (goal 050): the API gates
 * `/v1/review-tasks*` behind `X-Admin-Token`. The token comes from the
 * SERVER-SIDE env `GOVFOLIO_ADMIN_TOKEN` — deliberately not `NEXT_PUBLIC_*`,
 * so it is never inlined into the client bundle (this module only runs in
 * server components / server actions). Absent env → no header sent → the
 * API's 401/403 envelope surfaces honestly as `ApiError` (fail closed, no
 * fake state).
 */
function adminHeaders(): Record<string, string> {
  const token = process.env.GOVFOLIO_ADMIN_TOKEN;
  return token !== undefined && token !== "" ? { "x-admin-token": token } : {};
}

/** GET with If-None-Match; a 304 is served from the process-local body cache. */
async function conditionalGet(
  url: string,
  extraHeaders: Record<string, string> = {},
): Promise<{ status: number; body: string }> {
  const cached = etagCache.get(url);
  const headers = new Headers({ accept: "application/json", ...extraHeaders });
  if (cached) {
    headers.set("if-none-match", cached.etag);
  }
  // No explicit fetch cache mode: Next's default leaves data uncached
  // WITHOUT opting the route into dynamic rendering (an explicit `no-store`
  // would, killing the ISR s-maxage headers). Page/route-level `revalidate`
  // owns output caching; the ETag layer below makes each revalidation a
  // cheap 304 when nothing changed.
  const res = await fetch(url, { headers });
  if (res.status === 304 && cached) {
    return { status: 200, body: cached.body };
  }
  const body = await res.text();
  const etag = res.headers.get("etag");
  if (res.ok && etag) {
    if (etagCache.size >= ETAG_CACHE_MAX) {
      const oldest = etagCache.keys().next();
      if (!oldest.done) {
        etagCache.delete(oldest.value);
      }
    }
    etagCache.set(url, { etag, body });
  }
  return { status: res.status, body };
}

function buildUrl(
  path: string,
  pathParams: Record<string, string>,
  query: Record<string, string | number | undefined>,
): string {
  const filled = path.replace(/\{(\w+)\}/g, (_, name: string) => {
    const value = pathParams[name];
    if (value === undefined) {
      throw new Error(`missing path parameter ${name} for ${path}`);
    }
    return encodeURIComponent(value);
  });
  const url = new URL(filled, apiBaseUrl());
  for (const [key, value] of Object.entries(query)) {
    if (value !== undefined) {
      url.searchParams.set(key, String(value));
    }
  }
  return url.toString();
}

async function apiGet<P extends GetPath>(
  path: P,
  pathParams: Record<string, string> = {},
  query: GetQuery<P> | Record<string, never> = {},
  extraHeaders: Record<string, string> = {},
): Promise<GetOk<P>> {
  const url = buildUrl(path, pathParams, query as Record<string, string | number | undefined>);
  const { status, body } = await conditionalGet(url, extraHeaders);
  if (status >= 200 && status < 300) {
    return JSON.parse(body) as GetOk<P>;
  }
  throw apiErrorFrom(status, body);
}

/** Parses the consistent error envelope; falls back honestly when it isn't one. */
function apiErrorFrom(status: number, body: string): ApiError {
  let code = "unknown";
  let message = `API responded ${status}`;
  try {
    const parsed = JSON.parse(body) as ErrorBody;
    code = parsed.error.code;
    message = parsed.error.message;
  } catch {
    // non-envelope error body; keep the fallback code/message
  }
  return new ApiError(status, code, message);
}

export function listPoliticians(
  query: GetQuery<"/v1/politicians"> = {},
): Promise<PoliticianPage> {
  return apiGet("/v1/politicians", {}, query);
}

export function politicianProfile(id: string): Promise<PoliticianProfile> {
  return apiGet("/v1/politicians/{id}", { id });
}

export function politicianRecords(
  id: string,
  query: GetQuery<"/v1/politicians/{id}/records"> = {},
): Promise<RecordPage> {
  return apiGet("/v1/politicians/{id}/records", { id }, query);
}

export function listRecords(query: GetQuery<"/v1/records"> = {}): Promise<RecordPage> {
  return apiGet("/v1/records", {}, query);
}

export function getRecord(id: string): Promise<RecordDetail> {
  return apiGet("/v1/records/{id}", { id });
}

export function listJurisdictions(): Promise<Jurisdiction[]> {
  return apiGet("/v1/jurisdictions");
}

export function listRegimes(): Promise<Regime[]> {
  return apiGet("/v1/regimes");
}

export function search(q: string): Promise<SearchResults> {
  return apiGet("/v1/search", {}, { q });
}

// ---------- reviewer surface (goal 041; design §7.2) ----------

export function listReviewTasks(
  query: GetQuery<"/v1/review-tasks"> = {},
): Promise<ReviewQueuePage> {
  return apiGet("/v1/review-tasks", {}, query, adminHeaders());
}

export function getReviewTask(id: string): Promise<ReviewTaskDetail> {
  return apiGet("/v1/review-tasks/{id}", { id }, {}, adminHeaders());
}

export function reviewTaskAudit(id: string): Promise<ReviewAuditEntry[]> {
  return apiGet("/v1/review-tasks/{id}/audit", { id }, {}, adminHeaders());
}

type ResolveOk =
  paths["/v1/review-tasks/{id}/resolve"]["post"]["responses"][200]["content"]["application/json"];

/**
 * Resolves one review task — the ONLY door adjudication goes through (the
 * API's resolve endpoint onto pipeline promote; the UI never fabricates
 * state). Non-2xx surfaces as `ApiError` (409 = already resolved).
 */
export async function resolveReviewTask(
  id: string,
  input: ResolveInput,
): Promise<ResolveResponse> {
  const url = buildUrl("/v1/review-tasks/{id}/resolve", { id }, {});
  const res = await fetch(url, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      accept: "application/json",
      ...adminHeaders(),
    },
    body: JSON.stringify(input),
    cache: "no-store",
  });
  const body = await res.text();
  if (res.ok) {
    return JSON.parse(body) as ResolveOk;
  }
  throw apiErrorFrom(res.status, body);
}
