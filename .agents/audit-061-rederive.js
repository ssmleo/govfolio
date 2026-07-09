// AUDIT 061 — independent re-derivation of uk_commons_register expecteds.
// Rules transcribed BY THE AUDITOR from docs/regimes/uk_commons_register.md
// (§3.1 asset_class, §3.4 R1-R4 value, §3.5 owner, §3.6 dates, §4 silver,
// §5/§5.1 details+gold, §6 confidence) + MANIFEST conformance id constants.
// Deliberately does NOT read the adapter's src/.
"use strict";
const fs = require("fs");
const path = require("path");

const ROOT = "C:/projects/govfolio.io/crates/adapters/uk_commons_register/fixtures";
const CASES = [
  "shareholding_open_ended",
  "donation_exact",
  "land_no_value",
  "employment_pair_parent",
  "employment_pair_child",
];

// MANIFEST conformance id constants (contract, not derivation)
const REGIME_ID = "0GBRREG0000000000000000001";
const POLITICIAN_IDS = {
  4051: "0GBRMBR0000000000000000001",
  4651: "0GBRMBR0000000000000000002",
  5403: "0GBRMBR0000000000000000003",
  4521: "0GBRMBR0000000000000000004",
};
const FILING_IDS = {
  15475: "0GBRFNG0000000000000015475",
  15923: "0GBRFNG0000000000000015923",
  15854: "0GBRFNG0000000000000015854",
  15914: "0GBRFNG0000000000000015914",
  15915: "0GBRFNG0000000000000015915",
};

// §3.1: api category id -> asset_class. Only cat 7 (api id 8) equity,
// only cat 6 (api id 7) real_estate, everything else other.
function assetClass(catId) {
  if (catId === 8) return "equity";
  if (catId === 7) return "real_estate";
  return "other";
}

const THRESH_OPEN = "(ii) Other shareholdings, valued at more than £70,000";
const THRESH_PCT = "(i) Shareholdings: over 15% of issued share capital";

// §3.4 rules, in order, first match wins. Returns {value, value_source}.
function deriveValue(doc) {
  const fields = doc.fields || [];
  // R1: top-level Field name="Value" type="Decimal" WITH typeInfo.currencyCode
  const v = fields.find(
    (f) =>
      f.name === "Value" &&
      f.type === "Decimal" &&
      f.typeInfo &&
      typeof f.typeInfo.currencyCode === "string"
  );
  if (v) {
    if (v.typeInfo.currencyCode !== "GBP")
      throw new Error("unmapped currency " + v.typeInfo.currencyCode);
    return {
      value: { low: v.value, high: v.value, currency: "GBP" },
      value_source: "value_field",
    };
  }
  // R2: category 7 shareholdings (api id 8)
  if (doc.category.id === 8) {
    const t = fields.find((f) => f.name === "ShareholdingThreshold");
    const s = t ? t.value : null;
    if (s === THRESH_OPEN)
      return {
        value: { low: "70000.00", high: null, currency: "GBP" },
        value_source: "shareholding_threshold",
      };
    if (s === THRESH_PCT) return { value: null, value_source: "none" };
    throw new Error("R2c reject: unknown threshold " + JSON.stringify(s));
  }
  // R3: Donors[] (category 4 visits) — none in these fixtures
  const donors = fields.find((f) => f.name === "Donors");
  if (donors) throw new Error("R3 unexpected in fixtures");
  // R4: no money field
  return { value: null, value_source: "none" };
}

// §3.5 owner map
function deriveOwner(doc) {
  const fields = doc.fields || [];
  const catId = doc.category.id;
  if (catId === 8) {
    const h = fields.find((f) => f.name === "HeldOnBehalfOf");
    if (h && h.value !== null) throw new Error("HeldOnBehalfOf non-null: unknown+review");
    return "self";
  }
  if (catId === 7) {
    const s = fields.find((f) => f.name === "IsSoleOwner");
    if (!s || typeof s.value !== "boolean") throw new Error("IsSoleOwner missing");
    return s.value ? "self" : "joint";
  }
  if (catId === 10 || catId === 11) return null; // categories 9/10 family
  return "self";
}

// §6 confidence: 1.00; -0.02 registrationDate null; -0.05 multi-donor sum
function deriveConfidence(doc) {
  let c = 1.0;
  if (doc.registrationDate === null) c -= 0.02;
  return c;
}

// §5 details.fields flatten: typeInfo.currencyCode hoisted; explicit nulls
function flattenField(f) {
  return {
    name: f.name,
    description: f.description === undefined ? null : f.description,
    type: f.type,
    currency_code: f.typeInfo && f.typeInfo.currencyCode ? f.typeInfo.currencyCode : null,
    value: f.value === undefined ? null : f.value,
    values: f.values === undefined ? null : f.values,
  };
}

function deriveSilver(doc) {
  return [
    {
      payload: {
        interest_id: doc.id,
        row_ordinal: 1,
        version: doc.updatedDates.length,
        parent_interest_id: doc.parentInterestId,
        category_id: doc.category.id,
        category_number_raw: doc.category.number,
        category_name_raw: doc.category.name,
        member_id: doc.member.id,
        member_name_raw: doc.member.nameDisplayAs,
        member_list_name_raw: doc.member.nameListAs,
        member_from_raw: doc.member.memberFrom,
        party_raw: doc.member.party,
        house_raw: doc.member.house,
        summary_raw: doc.summary,
        registration_date_raw: doc.registrationDate,
        published_date_raw: doc.publishedDate,
        updated_dates_raw: doc.updatedDates,
        rectified: doc.rectified,
        rectified_details_raw: doc.rectifiedDetails,
        fields_raw: doc.fields, // VERBATIM
        extractor: "uk_commons_register/api@1",
      },
      confidence: deriveConfidence(doc),
    },
  ];
}

function deriveGold(doc) {
  // §3.8 checks the auditor re-asserts on the inputs themselves:
  if (doc.member.house !== "Commons") throw new Error("house != Commons");
  if (doc.category.type !== "Commons") throw new Error("category.type != Commons");
  if (doc.summary === "") throw new Error("empty summary reject");
  const { value, value_source } = deriveValue(doc);
  const threshField =
    doc.category.id === 8
      ? doc.fields.find((f) => f.name === "ShareholdingThreshold")
      : null;
  return [
    {
      filing_id: FILING_IDS[doc.id],
      politician_id: POLITICIAN_IDS[doc.member.id],
      regime_id: REGIME_ID,
      instrument_id: null,
      asset_description_raw: doc.summary,
      record_type: "interest",
      asset_class: assetClass(doc.category.id),
      side: null,
      transaction_date: null,
      as_of_date: null,
      notified_date: doc.registrationDate,
      value: value,
      owner: deriveOwner(doc),
      extraction_confidence: deriveConfidence(doc),
      extracted_by: "uk_commons_register/api@1",
      fingerprint: null,
      details: {
        interest_id: doc.id,
        version: doc.updatedDates.length,
        parent_interest_id: doc.parentInterestId,
        category_id: doc.category.id,
        category_number: doc.category.number,
        category_name: doc.category.name,
        member_id: doc.member.id,
        registration_date: doc.registrationDate,
        published_date: doc.publishedDate,
        updated_dates: doc.updatedDates,
        rectified: doc.rectified,
        rectified_details: doc.rectifiedDetails,
        shareholding_threshold_raw: threshField ? threshField.value : null,
        value_source: value_source,
        fields: doc.fields.map(flattenField),
      },
    },
  ];
}

// key-order-insensitive deep diff, both directions
function diff(pathStr, a, b, out) {
  if (a === b) return;
  const ta = Object.prototype.toString.call(a);
  const tb = Object.prototype.toString.call(b);
  if (ta !== tb) {
    out.push(`${pathStr}: type ${ta} vs ${tb} (${JSON.stringify(a)} vs ${JSON.stringify(b)})`);
    return;
  }
  if (Array.isArray(a)) {
    if (a.length !== b.length) out.push(`${pathStr}: array len ${a.length} vs ${b.length}`);
    const n = Math.min(a.length, b.length);
    for (let i = 0; i < n; i++) diff(`${pathStr}[${i}]`, a[i], b[i], out);
    return;
  }
  if (ta === "[object Object]") {
    const ka = Object.keys(a), kb = Object.keys(b);
    for (const k of ka) {
      if (!(k in b)) out.push(`${pathStr}.${k}: MISSING in committed expected`);
      else diff(`${pathStr}.${k}`, a[k], b[k], out);
    }
    for (const k of kb) if (!(k in a)) out.push(`${pathStr}.${k}: EXTRA in committed expected`);
    return;
  }
  out.push(`${pathStr}: ${JSON.stringify(a)} (re-derived) vs ${JSON.stringify(b)} (committed)`);
}

let fail = 0;
for (const c of CASES) {
  const raw = fs.readFileSync(path.join(ROOT, c, "input.json"));
  const doc = JSON.parse(raw.toString("utf8"));
  const silverExp = JSON.parse(fs.readFileSync(path.join(ROOT, c, "expected.silver.json"), "utf8"));
  const goldExp = JSON.parse(fs.readFileSync(path.join(ROOT, c, "expected.gold.json"), "utf8"));
  const dS = [], dG = [];
  diff("silver", deriveSilver(doc), silverExp, dS);
  diff("gold", deriveGold(doc), goldExp, dG);
  const ok = dS.length === 0 && dG.length === 0;
  if (!ok) fail++;
  console.log(`${c}: silver ${dS.length === 0 ? "MATCH" : "DIVERGE"}, gold ${dG.length === 0 ? "MATCH" : "DIVERGE"}`);
  for (const d of dS.concat(dG)) console.log("  " + d);
  // summary facts for the verdict table
  const g = deriveGold(doc)[0];
  console.log(
    `  rederived: cat=${doc.category.number} value=${g.value ? `${g.value.low}..${g.value.high === null ? "OPEN" : g.value.high} ${g.value.currency}` : "NULL"} src=${g.details.value_source} owner=${g.owner === null ? "NULL" : g.owner} notified=${g.notified_date} class=${g.asset_class}`
  );
}
console.log(fail === 0 ? "ALL 5 CASES: RE-DERIVATION MATCHES COMMITTED EXPECTEDS" : `${fail} CASE(S) DIVERGED`);
process.exitCode = fail === 0 ? 0 : 1;
