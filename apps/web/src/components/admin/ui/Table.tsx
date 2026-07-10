export interface TableColumn<T> {
  key: string;
  header: string;
  /** Right-align (header + cells) + tabular mono cells — set on every numeric/byte column. */
  numeric?: boolean;
  /** Prevent wrapping in the header and cells (dates, hashes, regime codes). */
  nowrap?: boolean;
  render: (row: T) => React.ReactNode;
}

export interface TableProps<T> {
  columns: ReadonlyArray<TableColumn<T>>;
  rows: readonly T[];
  getRowKey: (row: T) => string;
  emptyMessage?: string;
  /** Optional row-click handler. When passed, every `<tr>` becomes a keyboard-operable button (click/Enter/Space) that calls it with the row; omitted, rows render exactly as before. */
  onRowClick?: (row: T) => void;
}

// Dense data table: hairline rules, no zebra, numeric columns right-aligned
// in tabular mono. Cell fonts/colors live in the render callbacks. Scrolls
// its own overflow so a wide table never widens the page.
export function Table<T>({
  columns,
  rows,
  getRowKey,
  emptyMessage = "No rows.",
  onRowClick,
}: TableProps<T>) {
  if (rows.length === 0) {
    return (
      <p className="adm-muted" style={{ fontSize: "12.5px" }}>
        {emptyMessage}
      </p>
    );
  }

  const last = columns.length - 1;

  return (
    <div className="overflow-x-auto">
      <table className="w-full border-collapse">
        <thead>
          <tr>
            {columns.map((col, i) => (
              <th
                key={col.key}
                className={`adm-microlabel border-b border-[var(--adm-rule-strong)] ${
                  col.numeric ? "text-right" : "text-left"
                } ${col.nowrap ? "whitespace-nowrap" : ""}`}
                style={{ padding: i === last ? "8px 0" : "8px 14px 8px 0" }}
              >
                {col.header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr
              key={getRowKey(row)}
              onClick={onRowClick ? () => onRowClick(row) : undefined}
              onKeyDown={
                onRowClick
                  ? (event) => {
                      if (event.key === "Enter" || event.key === " ") {
                        event.preventDefault();
                        onRowClick(row);
                      }
                    }
                  : undefined
              }
              role={onRowClick ? "button" : undefined}
              tabIndex={onRowClick ? 0 : undefined}
              className={
                onRowClick
                  ? "cursor-pointer hover:bg-[var(--adm-gold-06)]"
                  : "hover:bg-[var(--adm-row-hover)]"
              }
              style={{ transition: "background .12s ease" }}
            >
              {columns.map((col, i) => (
                <td
                  key={col.key}
                  className={`border-b border-[var(--adm-rule)] ${
                    col.numeric ? "adm-num text-right" : "text-left"
                  } ${col.nowrap ? "whitespace-nowrap" : ""}`}
                  style={{ padding: i === last ? "10px 0" : "10px 14px 10px 0" }}
                >
                  {col.render(row)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
