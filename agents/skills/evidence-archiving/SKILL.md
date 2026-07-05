# skill: evidence-archiving
Purpose: cite nothing you have not archived
Load when: any claim about an external source is produced or checked
Core checklist:
- fetch -> store under docs/regimes/<x>/evidence/ sha-named -> reference {url, file} beside the claim
Anti-patterns: linking live URLs without snapshots; screenshots without source URL
Learnings (dated):
- 2026-07-05 (uk_commons_register): git eol conversion can corrupt sha-pinned
  evidence — with core.autocrlf=true a CRLF-bearing body is normalized at COMMIT
  (stored blob no longer matches the pin) and LF-bearing evidence is smudged to CRLF
  on Windows CHECKOUT (local re-hash fails). Evidence and fixture paths are marked
  `-text` in .gitattributes; verify with `git ls-files --eol` that pinned files show
  i/w agreement before pushing.
Write-back: deepen this file when the procedure teaches you something; same PR.
