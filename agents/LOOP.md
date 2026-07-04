# Loop protocol (Ralph-style)

Run shape: `while :; do cat agents/LOOP.md | claude ; done` (or /goal <file>).

Each iteration: run the orchestrator workflow (agents/workflows/orchestration.md).
Legacy simple mode below applies only if orchestration.md is absent.
1. Read `/CLAUDE.md`, then `agents/goals/000-INDEX.md`; pick the FIRST unchecked goal.
2. Read that goal file + every context pointer it lists.
3. If the goal is too big for one session: expand it into `docs/plans/<date>-<slug>.md`
   using the task format of the implementation plan, commit, and treat its Task 1 as the goal.
4. Do the smallest next step (one test, one impl, one command). TDD.
5. Run the goal's acceptance commands. Green? tick the goal in 000-INDEX.md.
6. Update the goal's own checklist section; commit with a conventional message.
7. If blocked on a human-only lane, write the question into the goal file under
   `## BLOCKED (human)` and move to the next unchecked goal.

Hard rules: branches only, never force-push, never edit generated files by hand,
never mark a goal done without its commands passing in this session.
