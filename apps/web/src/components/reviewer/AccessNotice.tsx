import type { ApiError } from "@/lib/api";

// The review surface is admin-gated (goal 050): the API answers 401/403
// unless the web server forwards a valid `X-Admin-Token` (server env
// `GOVFOLIO_ADMIN_TOKEN`). This notice surfaces the API's error envelope
// VERBATIM — no retry, no fabricated queue state (fail closed).
export function AccessNotice({ error }: { error: ApiError }) {
  return (
    <section className="access-notice" aria-label="Review surface unavailable">
      <h2>Review surface unavailable</h2>
      <p>
        The API refused this request (<span className="mono">{error.status}</span>):
      </p>
      <p data-testid="access-error">
        <span className="mono" data-testid="access-error-code">
          {error.code}
        </span>{" "}
        — <span data-testid="access-error-message">{error.message}</span>
      </p>
      <p className="muted">
        The reviewer console needs the admin token: set{" "}
        <span className="mono">GOVFOLIO_ADMIN_TOKEN</span> in the web server&apos;s
        environment to the API&apos;s <span className="mono">ADMIN_TOKEN</span>.
      </p>
    </section>
  );
}
