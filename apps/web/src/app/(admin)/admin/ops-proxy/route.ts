import { NextResponse } from "next/server";

import { ApiError, adminOverview } from "@/lib/api";

// Same-origin proxy for `SentinelTicker` (goal 091; renamed from
// `StatusStrip` in goal 094): the browser polls THIS
// route, never `/v1/admin/overview` directly — `adminOverview()` reads the
// server-only `GOVFOLIO_ADMIN_TOKEN` env (see `adminHeaders()` in lib/api),
// which must never reach client JS. Non-2xx from the API is forwarded
// honestly (status + the consistent error envelope), never swallowed into a
// fake 200 (fail closed).
export async function GET() {
  try {
    const data = await adminOverview();
    return NextResponse.json(data, { status: 200 });
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
