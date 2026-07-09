# Briefing for Brazilian accountant / cross-border tax lawyer consult

**Goal of this consult:** validate (or redirect) the plan to incorporate govfolio.io as
a Wyoming single-member LLC, given recent Brazilian tax rule changes, before filing
anything. See `README.md` in this folder for full decision context.

## Background
- Solo founder, Brazilian tax resident, operating a worldwide politician financial-
  disclosure tracking product (free tier + paid subscriptions + API).
- Base case: staying solo/bootstrap, no confirmed outside fundraising plans.
- Considering: Wyoming single-member LLC vs staying Brazilian (LTDA/SLU) vs Delaware
  C-Corp vs Estonia OÜ. Currently leaning Wyoming LLC for its stronger US-side liability
  shield (charging-order protection extended to single-member LLCs by statute, plus
  SPEECH Act protection against foreign defamation-judgment enforcement — relevant since
  the product publishes financial data about public officials and carries real
  defamation-suit tail risk).

## Specific things to confirm with the professional
1. **Lei 14.754/2023 (offshore CFC rules):** confirm the 15%-annual-tax-on-31-December
   mechanism for controlled foreign entities, and whether an operating SaaS/API business
   with >60% active income would normally have qualified for deferral under a "normal
   tax regime" jurisdiction.
2. **Solução de Consulta COSIT 56/2026 — precise framing, verified by direct read of
   the ruling PDF (not secondary summaries):** the decided case is a **multi-member**
   California family LLC (spouse + children), not a single-member disregarded entity.
   Its legal basis is **IN RFB 1.037/2010 art. 2º VII + Lei 9.430/1996**, not Lei
   14.754/2023 directly — Lei 14.754 only appears in the taxpayer's own framing, not in
   COSIT's reasoning. The ruling holds that a US LLC treated as tax-transparent/pass-
   through under US law qualifies as a "regime fiscal privilegiado," and that US
   member-level tax rates don't defeat that classification. It does **not** itself
   decide the 15%/60%-active-income-test/Dec-31 mechanics (those are separately true of
   Lei 14.754 in general, just not adjudicated by this SC). Ask the accountant directly:
   (a) does this reasoning plausibly extend to a **single-member** disregarded LLC like
   this one, or is that a real open question; (b) no Big Four or top-tier BR firm has
   published on this yet and existing commentary is split (one firm calls it settled,
   one calls it "excessively formalistic," one disputes it outright) — how much weight
   should an unsettled, non-mainstream-covered ruling get in a filing decision; (c) is
   there any realistic path to structure around triggering "regime fiscal privilegiado"
   classification at all, given the ruling turns on transparency/pass-through status
   specifically.
3. **Article 8 transparency election:** ask whether electing transparent treatment
   (reporting the LLC's underlying assets directly on the Brazilian tax return instead
   of the entity) is worth the operational overhead for a high-transaction-volume
   subscription business, given the election is irrevocable while the Member controls
   the entity.
4. **No US-Brazil tax treaty:** confirm this is still accurate (verified against the
   IRS's official treaty list as of 2026-07-06), and ask what double-taxation relief
   mechanisms (if any) apply when bringing LLC profit back to Brazil personally after
   it's already been taxed once under the CFC rule.
5. **Form 5472 (US):** confirm whether the accountant can prepare this annually, or
   recommend a US-side CPA who specializes in foreign-owned single-member LLC
   compliance — flag that the penalty for missing/late filing is $25,000 with no cap on
   continuing failure, so this needs a firm annual process, not best-effort.
6. **Sanity check the alternative:** given COSIT 56/26 may remove the LLC's main tax
   advantage (indefinite deferral), ask directly whether a Brazilian LTDA/SLU under
   Simples Nacional (Anexo III, Fator R-optimized, ~3-6% effective on export revenue)
   might now be the better net-tax outcome, accepting the weaker liability shield as a
   known tradeoff — i.e. is the liability-shield benefit of the LLC still worth its tax
   cost post-COSIT 56/26, in the accountant's professional judgment.

## What NOT to ask them (out of scope for this consult)
- Defamation/liability-shield analysis — that's a legal-risk judgment already reasoned
  through separately (SPEECH Act, charging-order protection); the accountant's job here
  is the tax math, not the litigation-risk tradeoff.
- Pricing/product questions — unrelated to this consult.
