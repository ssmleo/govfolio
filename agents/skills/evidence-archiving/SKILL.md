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
- 2026-07-06 (br surveyor, orchestrator intervention): a source can require
  personal_data_to_redact in the SURVEY (e.g. Brazil's TSE bulk data: unmasked CPF,
  voter-registration number, home addresses/plates/phone in free-text asset
  descriptions) while the specialist's own archived evidence file, fetched to PROVE
  that fact, still contains the live real values — evidence-archiving today has no
  step that checks this. Before a PII-flagging survey's evidence gets committed:
  grep the archived files for the flagged pattern (CPF-length digit runs, address/
  phone/plate markers) and replace the LIVE VALUE with an explicit
  `[REDACTED-BY-GOVFOLIO: ...]` placeholder that still demonstrates the STRUCTURAL
  claim (a column/field was populated with real-format data) without perpetuating a
  real person's sensitive data in git history forever. Record the redaction in the
  file's own retrieval.json sidecar, not just in AUTHORITY.md prose. This is
  distinct from and in addition to personal_data_to_redact (which governs the
  eventual Silver/Gold production pipeline, not the specialist's own evidence
  archive).
Write-back: deepen this file when the procedure teaches you something; same PR.
