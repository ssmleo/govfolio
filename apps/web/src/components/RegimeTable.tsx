import type { Regime } from "@/lib/api";
import { formatDate } from "@/lib/format";

// The scorecard row (design §7.3): what a regime discloses, how precisely,
// on what cadence, and with what statutory lag.
export function RegimeTable({ regimes }: { regimes: Regime[] }) {
  if (regimes.length === 0) {
    return <p className="empty">No disclosure regimes on file for this jurisdiction.</p>;
  }
  return (
    <table className="regimes">
      <thead>
        <tr>
          <th scope="col">Body</th>
          <th scope="col">Regime type</th>
          <th scope="col">Value precision</th>
          <th scope="col">Cadence</th>
          <th scope="col">Statutory lag</th>
          <th scope="col">In force</th>
        </tr>
      </thead>
      <tbody>
        {regimes.map((regime) => (
          <tr key={regime.id}>
            <td>
              {regime.source_url ? (
                <a href={regime.source_url} rel="noopener noreferrer">
                  {regime.body}
                </a>
              ) : (
                regime.body
              )}
            </td>
            <td>{regime.regime_type.replaceAll("_", " ")}</td>
            <td>{regime.value_precision}</td>
            <td>{regime.cadence ?? "—"}</td>
            <td className="cell-date">
              {regime.disclosure_lag_days !== null &&
              regime.disclosure_lag_days !== undefined
                ? `${regime.disclosure_lag_days} days`
                : "—"}
            </td>
            <td className="cell-date">
              since {formatDate(regime.effective_from)}
              {regime.effective_to ? `, until ${formatDate(regime.effective_to)}` : ""}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
