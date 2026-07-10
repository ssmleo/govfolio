# Adapters — folder context

Working on adapter `<x>`? Load the regime's SAF FIRST: `docs/regimes/<x>/AUTHORITY.md`
(directory form; legacy flat `docs/regimes/<x>.md` until goal 102). Write new learnings
back in the same PR — dated quirks-log entry, append-only (root CLAUDE.md → Memory).
Conformance: `cargo run -p pipeline --bin conformance -- <x>` · SAF check:
`cargo run -p pipeline --bin validate-survey -- <x>` (directory-form regimes).
