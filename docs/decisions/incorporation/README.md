# Incorporation, entity structure & billing — founder decision record

Founder decision 2026-07-05: incorporate govfolio.io as a **Wyoming single-member LLC**,
keep the already-shipped **Stripe-direct** billing (not a Merchant of Record), and lock
a 4-tier pricing ladder. Reached after two deep-research passes plus one adversarial-
verification pass (2026-07-06) on the Brazilian tax-risk claims. This record is the
rationale; supporting prep docs are in this folder.

## Decision: Wyoming single-member LLC

Chosen over Brazilian SLU, Delaware C-Corp, and Estonia OÜ.

- **Wyoming** is one of the few US states that extends charging-order-as-exclusive-
  remedy protection to *single-member* LLCs by statute — confirmed at **Wyo. Stat. Ann.
  § 17-29-503(g)**, which expressly covers "any judgment debtor who may be the sole
  member, dissociated member or transferee." Most states (see Florida's *Olmstead v.
  FTC*, 44 So. 3d 76 (Fla. 2010)) only protect multi-member LLCs this way — matters
  because the founder is solo.
- **SPEECH Act, 28 U.S.C. § 4102** (federal, identical in any US state): bars US courts
  from enforcing a foreign defamation judgment absent equivalent free-speech
  protections, and separately blocks enforcement against "interactive computer service"
  providers unless the judgment is consistent with 47 U.S.C. §230 as if the content
  originated domestically. Relevant because the product's core occupational hazard is a
  defamation claim from a tracked politician in a claimant-friendly jurisdiction
  (UK/Australia libel-tourism risk). Brazilian SLU and Estonia OÜ have no equivalent
  shield.
- **Estonia OÜ ruled out**: EDPB Opinion 04/2024 requires an EU "main establishment" to
  actually hold decision-making power to get GDPR's One-Stop-Shop benefit — doesn't
  apply when real decision-making stays with a Brazil-based solo founder. No GDPR
  benefit, no defamation shield, higher cost than Wyoming.
- **Delaware C-Corp deferred, not ruled out**: double taxation (21% federal + up to 30%
  dividend withholding, no US-Brazil tax treaty — confirmed via IRS's official treaty
  list) only justified once real outside fundraising exists. Sequencing: start Wyoming
  LLC, do a "Delaware Flip" conversion later if a term sheet materializes.

### Known cost of this choice — corrected 2026-07-06 after adversarial verification

Three independent research passes each retrieved and read the actual **Solução de
Consulta COSIT nº 56/2026** ruling PDF (not secondary summaries). Corrected findings:

- The decided case is a **multi-member** California family LLC (spouse + children), not
  a single-member disregarded entity. The reasoning is broad enough to plausibly extend
  to single-member LLCs, but that extension is **not itself a decided fact**.
- Its actual legal basis is **IN RFB 1.037/2010 art. 2º VII + Lei 9.430/1996** — Lei
  14.754/2023 appears only in the taxpayer's own framing, not in COSIT's reasoning. The
  ruling does not itself decide the 15%-rate / 60%-active-income-test / Dec-31 timing
  mechanics; those are separately true of Lei 14.754/2023 in general (confirmed against
  official statutory text) but not adjudicated by this specific ruling.
- **Not novel**: IN 1.037/2010 has classified this kind of structure since 2010; a 2018
  ruling (SC Cosit 218/2018) already touched non-resident scope.
- **No Big Four or top-tier Brazilian firm** (PwC/Deloitte/EY/KPMG, Mattos Filho,
  Machado Meyer, TozziniFreire, Pinheiro Neto, Demarest, Lefosse, Cascione) has
  published on it despite targeted searches. Existing commentary (2-3 boutique/columnist
  voices) is genuinely split: one calls it settled/binding, one calls the reasoning
  "excessively formalistic" ("o debate permanece aberto"), one disputes its validity
  outright.
- **No litigation/CARF/STJ challenge found** — absence-of-evidence only; the ruling is
  ~3 months old at last check.

**Net read:** the working assumption — that a transparent Wyoming LLC likely triggers
Brazilian "regime fiscal privilegiado" classification, and therefore Lei 14.754's
15%-annual-taxation-regardless-of-distribution mechanism — remains the most plausible
planning assumption, but is genuinely less certain than a first pass suggested. This is
a live, unsettled interpretation, not a settled cost. **Does not replace the accountant
consult below** — if anything, reinforces why it's needed.

## Decision: Stripe direct, not a Merchant of Record

The repo had already shipped a Stripe-direct billing integration (webhook/client seam,
usage-based quota tiers — `agents/goals/050-productization.md`, design.md §pricing)
before this pricing research ran; the research was initially blind to that and
recommended a MoR (Paddle/Lemon Squeezy) instead, reasoning that a MoR absorbs global
VAT/GST/sales-tax registration+remittance burden that Stripe-direct leaves on the
entity. Resolved 2026-07-05 in favor of keeping Stripe direct, after checking how the
two closest comps actually operate:

- **Quiver Quantitative** ToS: only addresses taxes generically ("added to your total"),
  no VAT/GST/MoR mention.
- **Unusual Whales** ToS: explicitly states *they* collect and remit US state sales tax
  directly ("to the appropriate state agency") — no VAT/GST mentioned anywhere.

Neither discloses using a MoR. Read as: comps tolerate the low-enforcement-probability
foreign-tax-compliance gap rather than solving it upfront via a MoR. Founder matched
that risk posture. **VAT/GST posture** (still open, distinct decision — see
`stripe-config-and-vat-posture.md`): enable Stripe Tax for US sales tax; do not
proactively register for EU/UK/AU VAT/GST at launch; revisit once real non-US paying
volume exists.

## Decision: 4-tier pricing ladder

| Tier | Price | Gate |
|---|---|---|
| Free | $0 | 48–72h delay, all jurisdictions, full historical search |
| Individual | $15/mo or $120/yr | Real-time alerts, all jurisdictions bundled (not split by geography), CSV export, no API, no commercial-use rights |
| API | $49/mo or $499/yr | Hard rate limit ~2,000 calls/day (429 on exceed — already shipped, no metered billing) |
| Enterprise | Custom quote | "Contact us" only — not counted as year-1 revenue |

Rationale: undercuts Quiver Quantitative ($30/mo) and Unusual Whales ($50/mo) entry
tiers deliberately, since retail willingness-to-pay for non-US political data
specifically is unproven — worldwide coverage is a marketing differentiator for retail,
not a retail price lever. It IS a genuine price lever for institutional/compliance
buyers (AML/KYC/ESG teams value one API over five scrapers as risk mitigation) — that's
what the Enterprise tier is for. Comp benchmarks used: Quiver Quantitative ~$2M ARR / 5k
paying subs / 0.66% free→paid conversion / ~$33 ARPU; Unusual Whales' 15-minute
free-tier delay as the proven attrition-to-paid lever; Autopilot ties revenue to
AUM/conviction rather than raw signups. Stripe's fee structure (~2.9%+$0.30 domestic,
~+2% international) is much lighter than a MoR's (~5%+$0.50), so the entry tier has more
room than MoR-based fee-floor math would suggest — no strict need for annual-only
billing at $15/mo.

## Still open — human-lane (founder action, not loop-executable)

None of these are agent-executable: entity filing, EIN, and bank-account opening all
require the founder's own signature/passport/SSN-ITIN as a hard external constraint,
independent of automation policy. Tracked here, not as an `agents/goals/*.md` item.

1. **Accountant / cross-border tax lawyer consult** — before filing anything. See
   `accountant-briefing.md`.
2. **File WY Articles of Organization + registered agent.**
3. **Apply for EIN** (no SSN/ITIN — IRS international phone line or fax/mail). See
   `ein-prep.md`.
4. **Operating Agreement** — draft ready, needs review + signing. See
   `operating-agreement-draft.md`.
5. **US business bank account** (Mercury primary, Wise backup).
6. **Annual compliance calendar** — WY Annual Report (~$60/yr) + Form 5472 + pro-forma
   1120 (CPA-prepared; $25k penalty risk, uncapped continuing failure, if missed/late).
7. **Media liability / E&O insurance quote.** See `insurance-shortlist.md`.
8. **Configure Stripe pricing tiers + confirm VAT/GST posture.** See
   `stripe-config-and-vat-posture.md`.

Public launch/billing should not go live until: entity + bank account exist, Stripe
tiers are configured, and the legal/methodology PUBLIC copy required by
`docs/runbooks/launch-checklist.md` §5 is drafted and founder-approved (ToS,
methodology pages, corrections policy, privacy policy) — ideally with insurance in
place. Cross-reference: `docs/runbooks/launch-checklist.md` §5 (pricing copy) and §7
(entity formation, added alongside this record).
