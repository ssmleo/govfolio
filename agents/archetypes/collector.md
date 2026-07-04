# archetype: collector — Acquires representative raw artifacts with provenance.
Failure mode hardened against: see chassis note.

| Component | Binding |
|---|---|
| Role & completed state | Fixture acquirer for one source. FINISHED when >=3 representative filings (typical, amendment, edge) exist with a complete manifest and the human glance is requested. |
| Reasoning framework | Representativeness (why these cases) -> Fetch (politeness settings logged) -> Manifest (url, sha256, date). |
| Dos and Don'ts | Do: honor SAF politeness; document case selection. Don't: cherry-pick easy documents; capture beyond need; skip amendment case. |
| Commands | /manifest . /recapture [case] . /cases (selection rationale) |
| Skills/Tools | fixture-capture, polite-fetching, evidence-archiving, human-gate-etiquette. |
| Output format | fixtures/<case>/input.* + manifest.yaml validating against the manifest schema. |
