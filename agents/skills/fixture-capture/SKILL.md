# skill: fixture-capture
Purpose: representative, provenanced samples
Load when: SAMPLE phase or extending test coverage
Core checklist:
- pick typical + amendment + edge -> record manifest {url, sha256, date, politeness} -> flag human gate for expected outputs
Anti-patterns: cherry-picking easy docs; missing amendment case
Learnings (dated):
- 2026-07-05: scanned-fixture transcription needs TWO independent RENDERERS, not two
  looks at one render. On Windows hosts without poppler, WinRT `Windows.Data.Pdf`
  (PowerShell, `RenderToStreamAsync` at 4x + System.Drawing region crops) is a true
  second engine beside the harness's native PDF read. Note: `Bitmap.Clone` throws
  "Out of memory" when the crop rect exceeds image bounds — it's a bounds bug, not memory.
- 2026-07-05: before landing a NEW fixture case dir, grep for what auto-discovers the
  fixtures root (conformance `run_cases` runs every subdir; e2e tests may assert exact
  case counts). A case whose parser doesn't exist yet goes in a sibling dir
  (`fixtures-llm/`, cf. `fixtures-broken/`) so intended-TDD-red never pushes a red CI.
- 2026-07-05: sizing candidates by HEAD Content-Length is a cheap politeness-friendly
  proxy for scan page count (24–56 KB spread cleanly separated 1-page from multi-page
  paper PTRs); probe several, GET one.
Write-back: deepen this file when the procedure teaches you something; same PR.
