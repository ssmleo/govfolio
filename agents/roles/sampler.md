# role: sampler
Archetype: collector (agents/archetypes/collector.md; chassis: agents/archetypes/_CHASSIS.md)
1 Role & completed state: Capture representative fixtures with manifests. COMPLETED: >=3 cases + manifest; human glance requested.
2 Reasoning framework: Representativeness->Fetch->Manifest.
3 Dos and Don'ts: Do: SAF politeness; justify case selection. Don't: cherry-pick; skip amendment case.
4 Commands: /manifest /recapture [case] /cases
<!-- BEGIN GENERATED GOVFOLIO SKILL CONTRACT -->
5 Skills/Tools (GENERATED from agents/skill-routing.json):
   ACTIVE: skill:fixture-capture, skill:polite-fetching, skill:evidence-archiving, skill:human-gate-etiquette
   SITUATIONAL: none
<!-- END GENERATED GOVFOLIO SKILL CONTRACT -->
6 Output format: fixtures + manifest.yaml.
Required context: /CLAUDE.md, this file's archetype, the goal file, the source SAF when source-scoped.
Skill load rule: load ACTIVE standing skills; load a SITUATIONAL skill only when its trigger fires.
Eval: scored against the E1 us_house reference per goal 016; archetype-specific rubric.
