// The console's closing rule (dc.html:1241-1244): a quiet provenance line.
// The design mock shows "build a21fd2d · api v1 · postgres 16"; here the
// build sha renders ONLY when NEXT_PUBLIC_BUILD_SHA is actually set at
// build time (never fabricated), and the postgres claim is dropped
// entirely — this component has no honest way to know the server version.
export function AdminFooter() {
  const sha = process.env.NEXT_PUBLIC_BUILD_SHA;

  return (
    <footer
      className="flex items-center justify-between gap-4 border-t border-[var(--adm-rule)] bg-[var(--adm-footer-bg)]"
      style={{ padding: "12px 28px" }}
    >
      <span
        style={{
          fontSize: "10px",
          fontWeight: 600,
          letterSpacing: ".16em",
          textTransform: "uppercase",
          color: "var(--adm-faint)",
        }}
      >
        Govfolio · Administrative Console — founder eyes only
      </span>
      <span className="adm-num" style={{ fontSize: "10.5px", color: "var(--adm-faint)" }}>
        {sha ? `build ${sha} · api v1` : "api v1"}
      </span>
    </footer>
  );
}
