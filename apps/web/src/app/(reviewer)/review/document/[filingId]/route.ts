import { NextResponse } from "next/server";

import { ApiError, adminFilingDocument } from "@/lib/api";

interface Params {
  params: Promise<{ filingId: string }>;
}

// Same-origin proxy for BronzeDocument's iframe/fallback link (design §7.2):
// the browser fetches THIS route, never the ADMIN-gated
// /v1/admin/filings/{id}/document directly — adminFilingDocument() reads the
// server-only GOVFOLIO_ADMIN_TOKEN env (see adminHeaders() in lib/api),
// which must never reach client JS. Reviewers need to see filings the
// moment they're ingested, not 24h later — the PUBLIC
// /v1/filings/{id}/document route's free-tier embargo would 404 a fresh
// filing, which is exactly what the review queue exists to adjudicate.
// Mirrors admin/ops-proxy/route.ts's pattern, but streams binary bytes
// (the archived document) instead of JSON. Non-2xx from the API is
// forwarded honestly as the consistent error envelope, never swallowed into
// a fake 200 (fail closed).
export async function GET(_request: Request, { params }: Params) {
  const { filingId } = await params;
  try {
    const { body, contentType } = await adminFilingDocument(filingId);
    return new NextResponse(body, {
      status: 200,
      headers: {
        "content-type": contentType,
        "x-content-type-options": "nosniff",
      },
    });
  } catch (error) {
    if (error instanceof ApiError) {
      return NextResponse.json(
        { error: { code: error.code, message: error.message } },
        { status: error.status },
      );
    }
    throw error;
  }
}
