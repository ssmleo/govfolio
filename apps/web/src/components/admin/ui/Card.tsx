export interface CardProps {
  /** Small caps label above the title, e.g. a section code ("A2", "D1"). */
  eyebrow?: string;
  title?: string;
  /** Optional right-aligned slot next to the title (e.g. a period toggle). */
  action?: React.ReactNode;
  children: React.ReactNode;
  className?: string;
}

// The base panel of the instrument panel: a flat surface, hairline border,
// no shadow (a drop shadow reads as generic SaaS chrome on this ground).
export function Card({ eyebrow, title, action, children, className }: CardProps) {
  const hasHead = eyebrow !== undefined || title !== undefined;
  return (
    <section
      className={`rounded-sm border border-[var(--adm-rule)] bg-[var(--adm-surface)] p-4 ${className ?? ""}`}
    >
      {hasHead && (
        <div className="mb-3 flex items-start justify-between gap-3">
          <div>
            {eyebrow !== undefined && <p className="adm-eyebrow mb-1">{eyebrow}</p>}
            {title !== undefined && <h3>{title}</h3>}
          </div>
          {action !== undefined && <div>{action}</div>}
        </div>
      )}
      {children}
    </section>
  );
}
