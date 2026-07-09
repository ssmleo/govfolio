# EIN application prep (Form SS-4, no SSN/ITIN route)

**Prerequisite:** WY Articles of Organization must be filed and approved first — the
LLC's legal name and formation date/filing number are needed before applying.

## Route: IRS International Applicant phone line (faster than fax/mail)
- Number: **+1 267-941-1099** (not toll-free). Hours: Mon-Fri, 6am-11pm Eastern Time.
  BRT is UTC-3, US Eastern is UTC-4/-5 depending on DST — roughly 1-2hr offset, so most
  of the window lands in normal Brazilian business hours.
- No SSN/ITIN needed via this route — the agent completes Form SS-4 over the phone and
  issues the EIN verbally, followed by written confirmation (Form CP 575) by mail.
- Expect hold times; call right when the line opens for the best odds.

## Fallback: fax or mail Form SS-4
- Download Form SS-4 from irs.gov, fill by hand/PDF, fax to the number listed in the
  current SS-4 instructions for international applicants (check irs.gov for the current
  fax number — it has changed in the past). Turnaround: ~4-6 weeks by fax, longer by
  mail.

## Fields needed ready (for either route)
- Legal name of entity: [LLC NAME], LLC
- Trade name (if different from legal name): govfolio.io (if used as a DBA)
- Mailing address: the registered agent's address, or the founder's own if preferring
  IRS mail to reach them directly
- County/state of principal business: Wyoming
- Responsible party name + foreign tax ID: founder's full legal name + Brazilian CPF
- Type of entity: "Foreign-owned U.S. Disregarded Entity" (single-member LLC not
  electing corporate tax treatment)
- Reason for applying: "Started new business"
- Date business started: WY Articles of Organization filing date
- Principal business activity: "Data processing / online publishing" or "Software
  publishing" (closest NAICS-aligned description of a SaaS/API data product)
- Principal product/service: "Financial disclosure data aggregation and alerting"

## After the EIN is issued
- Needed immediately for: Mercury/Wise bank account application, and eventually the
  Stripe account (already exists per repo — confirm it's tied to the LLC's EIN, not a
  personal one, before real money starts flowing through it).
