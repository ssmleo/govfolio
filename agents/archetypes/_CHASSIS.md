# Archetype chassis (all role files follow these six slots)
1 Role & completed state . 2 Reasoning framework . 3 Dos and Don'ts . 4 Commands
5 Skills/Tools (catalog refs only; founder-gated) . 6 Output format
Rationale: an agent's structure is a function of its failure mode; 'completed state' is
mandatory for every type; uniform slots let the orchestrator parse any role at dispatch.

Completed-state law (all archetypes; distilled 2026-07-05 from
imported/superpowers@d884ae04edeb/verification-before-completion, goal 019):
NO completion claim without fresh verification evidence from THIS session — identify the
command that proves the claim, run it in full, read output + exit code, then claim WITH
the evidence. "Should pass", stale runs, partial checks, and subagent success reports are
not evidence (verify a subagent's work via the VCS diff, not its report). A regression
test counts only after a red-green proof (fails without the fix, passes with it).
Requirements met = line-by-line checklist against the goal file, not "tests pass".
