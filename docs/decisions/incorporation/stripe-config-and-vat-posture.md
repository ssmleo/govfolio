# Stripe tier configuration + VAT/GST posture recommendation

## Tier configuration to create in Stripe
(Product/Price objects — names are suggestions, adjust to match the existing product UI
copy in apps/web.)

| Tier | Product name | Price | Billing interval | Notes |
|---|---|---|---|---|
| Free | — | $0 | — | No Stripe object needed, gated in-app |
| Individual | "govfolio Individual" | $15.00 | monthly | Real-time alerts, all jurisdictions, CSV export |
| Individual (annual) | "govfolio Individual — Annual" | $120.00 | yearly | ~33% discount vs monthly, matches comp-set annual-discount norms |
| API | "govfolio API" | $49.00 | monthly | Hard 2,000 calls/day limit, enforced by the existing rate-limit gate (`agents/goals/050-productization.md`) |
| API (annual) | "govfolio API — Annual" | $499.00 | yearly | |
| Enterprise | — | custom | — | "Contact us" only — no Stripe Price object yet, manual invoicing when it happens |

Given Stripe's fee structure (~2.9%+$0.30 domestic, ~+2% international surcharge) is
much lighter than a Merchant of Record's, there's more room than an MoR-based analysis
would suggest — a monthly-only entry tier at $15 is comfortably fine (fee take ~5-8%,
nowhere near the MoR "under $10-15 is irrational" trap). Annual pricing is still worth
offering for the discount/cashflow benefit, just not required to avoid fee absurdity.

## VAT/GST posture — recommendation
Given Quiver Quantitative and Unusual Whales (closest comps) show no evidence of
proactive VAT/GST registration in the EU/UK/Australia — both appear to only handle US
state sales tax directly — the pragmatic, comp-matched posture is:

1. **Turn on Stripe Tax for US sales tax only** (calculates/collects/remits per state
   economic nexus — Stripe can automate registration monitoring for US states as
   revenue grows).
2. **Do not proactively register for EU VAT / UK VAT / Australian GST** at launch.
   Charge the same flat price globally, no VAT line item added. This mirrors what the
   comps evidently do, and matches their apparent risk tolerance.
3. **Revisit if/when non-US revenue becomes real** (e.g. a clear, sustained volume of
   EU/UK/AU paying subscribers) — that's the point to register properly (via EU's OSS
   scheme, UK VAT registration for non-established sellers, AU GST for non-residents)
   rather than front-loading compliance cost against hypothetical revenue.
4. **This is a real, if low-probability, compliance gap being consciously accepted**,
   not a solved problem — flagging it explicitly rather than treating Stripe Tax without
   registration as fully compliant. Revisit this call once there's real international
   paying-customer volume, not on a fixed calendar date.
