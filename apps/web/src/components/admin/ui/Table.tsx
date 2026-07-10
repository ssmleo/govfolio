export interface TableColumn<T> {
  key: string;
  header: string;
  /** Right-align + tabular mono — set on every numeric/byte/hash column. */
  numeric?: boolean;
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
// in tabular mono. Scrolls its own overflow so a wide table never widens
// the page.
export function Table<T>({
  columns,
  rows,
  getRowKey,
  emptyMessage = "No rows.",
  onRowClick,
}: TableProps<T>) {
  if (rows.length === 0) {
    return <p className="text-sm text-[var(--adm-muted)]">{emptyMessage}</p>;
  }

  return (
    <div className="overflow-x-auto">
      <table className="w-full border-collapse text-sm">
        <thead>
          <tr>
            {columns.map((col) => (
              <th
                key={col.key}
                className={`whitespace-nowrap border-b border-[var(--adm-rule-strong)] py-1.5 pr-4 text-xs font-semibold text-[var(--adm-muted)] ${
                  col.numeric ? "text-right" : "text-left"
                }`}
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
              className={onRowClick ? "cursor-pointer hover:bg-[var(--adm-surface-sunken)]" : undefined}
            >
              {columns.map((col) => (
                <td
                  key={col.key}
                  className={`border-b border-[var(--adm-rule)] py-1.5 pr-4 align-top ${
                    col.numeric ? "adm-num text-right" : "text-left"
                  }`}
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
