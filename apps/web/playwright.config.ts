import { defineConfig, devices } from "@playwright/test";

import { ADMIN_TOKEN } from "./e2e/api";

// E2E runs against a production `next build && next start` on PORT, which in
// turn consumes the LOCALLY RUNNING, SEEDED govfolio API:
//   1. scripts/dev/pg-local.ps1 start        (Postgres on 5433)
//   2. cargo run -p worker --bin local       (seed: pipeline over fixtures)
//   3. ADMIN_TOKEN=govfolio-e2e-admin-dummy cargo run -p api   (API on :8080)
// Then: pnpm e2e (from the repo root) or pnpm --filter web e2e.
//
// The reviewer flows are admin-gated (goal 050): the API must be started with
// ADMIN_TOKEN set to the e2e dummy above (e2e/api.ts), and the web process
// gets the SAME value as GOVFOLIO_ADMIN_TOKEN below so its server-side client
// forwards X-Admin-Token on review-surface calls. Public flows stay
// unauthenticated by design.
const PORT = 3105;
const API_URL = process.env.GOVFOLIO_API_URL ?? "http://localhost:8080";

export default defineConfig({
  testDir: "./e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  use: {
    baseURL: `http://localhost:${PORT}`,
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: `pnpm build && pnpm exec next start --port ${PORT}`,
    url: `http://localhost:${PORT}`,
    timeout: 300_000,
    reuseExistingServer: !process.env.CI,
    env: {
      GOVFOLIO_API_URL: API_URL,
      GOVFOLIO_SITE_URL: `http://localhost:${PORT}`,
      GOVFOLIO_ADMIN_TOKEN: ADMIN_TOKEN,
    },
  },
});
