# 111 — autonomous-loop-hardening-r3

## Objective

Apply only the evidence-triggered Release 3 expansion in design §9. This goal is
registered but deliberately not executable until the metric gates are satisfied.

## Entry gate

Collect at least 20 applied receipts or 24 hours after Releases 0–2 are stable. Then:

- Add one producer only when projected utilization is ≤65%, queue p95 <15 minutes,
  and conflict rate <5%.
- Reduce producers when utilization >80% or queue p95 >30 minutes.
- Maximum seven producers; final `main` mutation remains singleton.
- Batch at most three path-disjoint receipts only if sustained demand exceeds serial
  capacity and conflict rate <2%.
- Build a separate semantic resolver only if conflict rate >5% or at least two
  receipts remain in bounded rework during a week. It never pushes or finalizes.

## Checklist

- [ ] Entry dataset exists (≥20 applied receipts or ≥24h)
- [ ] Utilization, queue p95, conflict rate, and CI p95 computed
- [ ] Only triggered features implemented and tested
- [ ] Scaling decision/evidence recorded and merged to main

## BLOCKED (human)

(empty — metric-gated, not human-gated)

