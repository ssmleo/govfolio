import { defineConfig, devices } from "@playwright/test";

// E2E runs against a production `next build && next start` on PORT, which in
// turn consumes the LOCALLY RUNNING, SEEDED govfolio API:
//   1. scripts/dev/pg-local.ps1 start        (Postgres on 5433)
//   2. cargo run -p worker --bin local       (seed: pipeline over fixtures)
//   3. cargo run -p api                      (API on :8080)
// Then: pnpm e2e (from the repo root) or pnpm --filter web e2e.
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
    },
  },
});
