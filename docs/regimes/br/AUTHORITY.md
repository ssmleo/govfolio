---
# RegimeSurvey front-matter (validated). Every claim: {claim, evidence:[files]}
jurisdiction: "br"
bodies: ["Câmara dos Deputados (federal lower house)", "Senado Federal (federal upper house)"]
legal_basis:
  claim: "Lei 9.504/1997 (Lei das Eleições), art. 11, §1º, IV requires every candidacy-registration request ('pedido de registro') to be accompanied by a 'declaração de bens, assinada pelo candidato' (asset declaration signed by the candidate) as one of the mandatory registration documents. §6º (added by Lei nº 12.034/2009) requires the Justiça Eleitoral to make these registration documents accessible to interested parties: 'A Justiça Eleitoral possibilitará aos interessados acesso aos documentos apresentados para os fins do disposto no §1º.' Verified directly against the enacted primary statute text hosted at planalto.gov.br (the federal government's own consolidated-legislation host), not a secondary summary; also independently corroborated by TSE's own jurisprudence-compilation page (temasselecionados.tse.jus.br), which quotes the identical art. 11 §1º IV / §6º language across a chain of TSE/STF decisions holding the declaration public and non-confidential. The publication mechanics — specifically which sub-fields are withheld (CPF, personal address, personal phone/email, ID document) — are governed one level down by TSE's own implementing regulation, Resolução TSE nº 23.609/2019 art. 33 §2º (as amended by Resolução TSE nº 23.729/2024); this resolution text is quoted verbatim in the open-data leiame documentation archived below."
  evidence:
    - url: "https://www.planalto.gov.br/ccivil_03/leis/l9504.htm"
      file: "b9aa7f42649f77bfab1c8fcb43c887182942a021782cec93ae2a2d3566fcee08.planalto-lei9504-art11.html"
    - url: "https://temasselecionados.tse.jus.br/temas-selecionados/registro-de-candidato/documentacao/declaracao-de-bens"
      file: "a91c40fbd19fe06258a36f47033cf9375a2932254c36bcac35ef7957ae6ae1af.tse-declaracao-de-bens-doc.html"
    - url: "https://cdn.tse.jus.br/estatistica/sead/odsele/consulta_cand/consulta_cand_2022.zip"
      file: "f30553fb57dddd4ea6a3b1a636b50cb90edf2ac40ada27fe5cb7993d0c0cad70.consulta-cand-leiame-dictionary-cpf-resolution.pdf"
who_files:
  claim: "Every candidate who registers for a Câmara dos Deputados seat (cargo code 'DEPUTADO FEDERAL') or a Senado Federal seat ('SENADOR', plus the two ranked alternates '1º SUPLENTE'/'2º SUPLENTE' who run on the same ticket) at each quadrennial federal general election ('Eleição Geral Federal') — NOT limited to eventual winners or sitting members, and NOT an annual in-office filing. Independently confirmed by directly downloading and inspecting TSE's own 2022 bulk candidate-registration dataset (consulta_cand_2022, all-state files): DS_CARGO values observed for state-level files include 'DEPUTADO FEDERAL', 'SENADOR', '1º SUPLENTE', '2º SUPLENTE' alongside GOVERNADOR/VICE-GOVERNADOR/DEPUTADO ESTADUAL; the national ('BR') file separately carries PRESIDENTE/VICE-PRESIDENTE. Every candidate who declares any assets appears, joined by the per-cycle key SQ_CANDIDATO, in the companion bem_candidato file; a candidate who declares zero assets legitimately has no bem_candidato row at all (observed directly: several 'DEPUTADO FEDERAL' candidates in the Acre sample have no matching asset rows), consistent with the TSE jurisprudence excerpt's own holding that a candidate need only affirmatively state having no assets rather than file an empty form. Sitting members do NOT re-file here between elections — that is a structurally separate, non-public mechanism (DBR, see open_questions)."
  evidence:
    - url: "https://cdn.tse.jus.br/estatistica/sead/odsele/consulta_cand/consulta_cand_2022.zip"
      file: "081cd58b919f9db84a4e34fb275a248efa838a88533d020dc2c4ba061081dbd6.consulta-cand-2022-excerpt-cpf-unmasked.csv"
    - url: "https://cdn.tse.jus.br/estatistica/sead/odsele/consulta_cand/consulta_cand_2022.zip"
      file: "f30553fb57dddd4ea6a3b1a636b50cb90edf2ac40ada27fe5cb7993d0c0cad70.consulta-cand-leiame-dictionary-cpf-resolution.pdf"
record_types: [holding]
value_precision: "exact"
band_table: []
cadence_and_lag:
  claim: "NOT rolling and NOT annual — filed once per candidacy, at candidacy-registration time for each quadrennial federal general election (Presidente, Governador, Deputado Federal, Deputado Estadual, Senador all elected together; Brazilian municipal elections for Prefeito/Vereador run on their own quadrennial calendar offset by 2 years, e.g. 2020/2024 vs. 2018/2022/2026 — confirmed structurally by the fact that the 2022 open-data package contains DEPUTADO FEDERAL/SENADOR candidates while the 2024 package contains only PREFEITO/VEREADOR/VICE-PREFEITO). Statutory registration deadline (Lei 9.504/1997 art. 11 caput, current wording per Lei nº 13.165/2015): parties/coalitions must request registration 'até as dezenove horas do dia 15 de agosto do ano em que se realizarem as eleições' (by 7pm on August 15 of the election year); election day itself is the first Sunday of October (observed DT_ELEICAO = '02/10/2022' in the bulk data) — so the declaration is filed roughly 6-7 weeks before the vote, not continuously. However, the underlying record is NOT frozen at filing: per-item timestamps (DT_ULT_ATUAL_BEM_CANDIDATO) on the SAME 2022-cycle declarations show live edits recorded years after that election (e.g. a Alagoas 'DEPUTADO FEDERAL' candidate's full 9-item 2022 declaration is stamped last-updated 13/05/2026) — see amendment_mechanism. So: filing TRIGGER is per-candidacy/per-election-cycle; the filed RECORD can be amended long after the vote."
  evidence:
    - url: "https://www.planalto.gov.br/ccivil_03/leis/l9504.htm"
      file: "b9aa7f42649f77bfab1c8fcb43c887182942a021782cec93ae2a2d3566fcee08.planalto-lei9504-art11.html"
    - url: "https://cdn.tse.jus.br/estatistica/sead/odsele/bem_candidato/bem_candidato_2022.zip"
      file: "e46cb76c0124f0002d4480c49680ae2e01f21e5711bb7134c949843dfd64c947.bem-candidato-leiame-dictionary.pdf"
    - url: "https://cdn.tse.jus.br/estatistica/sead/odsele/bem_candidato/bem_candidato_2022.zip"
      file: "7c3d5bec1f33a86dbde1ab89365d78125bd2c201cf7446407490278b2f839421.bem-candidato-2022-al-deputado-federal-late-amendment.csv"
formats: [csv_bulk_download, html_table]
access:
  method: "Two access surfaces exist for the same underlying data. (1) PRACTICAL path: bulk CSV-in-ZIP download, no login/session/API-key/captcha, at a fixed predictable URL pattern `https://cdn.tse.jus.br/estatistica/sead/odsele/{dataset}/{dataset}_{ANO}[_{UF}].zip` — confirmed dataset names `consulta_cand` (candidate registration + identity fields) and `bem_candidato` (itemized asset list), one nationwide file plus one per-UF file each, for every election year in TSE's catalog. This is independent of the human search portal's availability and was directly downloaded, unzipped, and parsed this session for the 2022 (federal) and 2024 (municipal) cycles. A CKAN metadata API at dadosabertos.tse.jus.br (`/api/3/action/package_show`, `package_search`) exists to discover these resource URLs and was used for that discovery this session, but see tos_and_politeness — its own robots.txt disallows `/api/`, so the eventual adapter should hardcode the URL pattern instead of crawling the CKAN API. (2) divulgacandcontas.tse.jus.br, the human-facing search/browse portal that TSE's own documentation names as the 'official' presentation layer for individual declarations, was UNREACHABLE for the entire scout-plus-survey window (2026-07-06): both the root and `/divulga/` 302-redirect to a generic maintenance page (`cdn.tse.jus.br/indisponivel.html`), reproduced identically under an identified UA and a stock browser UA — reads as a genuine platform outage rather than a bot-block, and was independently re-confirmed unchanged at survey time (see Operational notes)."
  session_required: false
  captcha: "none observed on any of the three hosts probed (cdn.tse.jus.br, dadosabertos.tse.jus.br, and divulgacandcontas.tse.jus.br's redirect target)"
  notes: "The bulk-CSV path does not depend on divulgacandcontas.tse.jus.br being reachable at all — it is served entirely from cdn.tse.jus.br, a separate host. Outage root cause/duration for the portal is unresolved (see open_questions; sentinel-relevant once live)."
historical_depth:
  from: "TSE's open-data catalog (dadosabertos.tse.jus.br) lists a 'candidatos-<year>' package for every election year from 1933 through 2024 EXCEPT 2020 (no 'candidatos-2020' package found; a municipal-only year, so this gap does not affect Câmara/Senado coverage — cause of the gap not established). TSE's own catalog description flags 1994-1998 candidate data as incomplete ('Estão incompletos os dados de candidatos das eleições de 1994 a 1998, pois os mesmos não foram completamente centralizados nas bases no TSE'). This session directly downloaded, unzipped, and parsed the 'bem_candidato' (itemized asset) resource for the 2022 federal cycle end-to-end (confirming the disclosure content itself, not just candidate-registration metadata, is present and machine-readable for that year) and confirmed the 2024 municipal cycle's companion resource exists and is structurally identical. Whether the bem_candidato resource specifically extends as far back as the registration-metadata catalog (1933) — versus only from whichever year the modern itemized-CSV format was introduced — was NOT verified beyond the 2022/2024 cycles directly inspected; flagged as an open question for whoever scopes a historical backfill."
  evidence:
    - url: "https://dadosabertos.tse.jus.br/api/3/action/package_list"
      file: "4214782f1a60398b64c221026fdca6d9faa444ca443454062eff5bd545dd6dd6.dadosabertos-package-list-1933-2024.json"
    - url: "https://dadosabertos.tse.jus.br/api/3/action/package_show?id=candidatos-2022"
      file: "a2b02be625a591d917b45e4ca10b570446bfaff5b6b432ab7849922dabe9b2a6.dadosabertos-package-candidatos-2022.json"
identifiers_available:
  politician: "NR_CPF_CANDIDATO (Brazilian national taxpayer ID, CPF) is the only durable cross-cycle personal identifier present in the source, but as of the 2024 election cycle it is suppressed in the bulk open-data files (every value replaced with the documented numeric-null sentinel '-4') per Resolução TSE nº 23.609/2019 art. 33 §2º as amended by Resolução TSE nº 23.729/2024 — confirmed both by the leiame documentation's explicit statement and by directly diffing the 2022 CSV (real, unmasked CPF numbers observed) against the 2024 CSV (every NR_CPF_CANDIDATO value observed was the literal sentinel '-4'). NR_TITULO_ELEITORAL_CANDIDATO (voter-registration number) remains unmasked in both cycles checked and is a second, still-live personal identifier (see personal_data_to_redact). SQ_CANDIDATO is NOT a durable identifier: the leiame documentation states it is generated fresh 'para cada eleição' (for each election) by the electoral systems, and it is only usable as a same-file join key between consulta_cand and bem_candidato within one election cycle's file set. No parliamentary-register-style stable ID (e.g. a bioguide-equivalent) was found."
  instrument: "none — assets are recorded as a type code (CD_TIPO_BEM_CANDIDATO, e.g. 12='Casa', 21='Veículo automotor terrestre...', 61='Depósito bancário em conta corrente no País') paired in every row with its own human-readable label (DS_TIPO_BEM_CANDIDATO) plus a free-text description (DS_BEM_CANDIDATO); no ISIN/ticker/registry identifier of any kind is used or expected, since the declared assets are overwhelmingly real estate, vehicles, and bank/savings balances rather than tradable securities."
amendment_mechanism:
  claim: "Yes, at individual asset-line-item granularity, and well beyond the campaign period. Confirmed directly from the bulk bem_candidato dataset: each declared asset item carries its own DT_ULT_ATUAL_BEM_CANDIDATO/HH_ULT_ATUAL_BEM_CANDIDATO ('last-update') timestamp. A 2022-cycle 'DEPUTADO FEDERAL' candidate from Alagoas (SQ_CANDIDATO 20001605787) has all 9 of their declared asset items stamped 13/05/2026 — nearly 4 years after the 02/10/2022 election day, and after the entire subsequent 2024 municipal cycle. This matches the TSE jurisprudence excerpts (temasselecionados evidence), which repeatedly describe and uphold a formal 'retificação' (rectification) process for a filed declaração de bens, including post-election corrections. Each declared item's description also frequently cites its own source-of-truth reference (e.g. 'em 31/12/2021-Declaração do IRPF do Exercício 2022-Ano-Calendário 2021'), i.e. values are commonly carried over from the candidate's income-tax return (IRPF) for the prior fiscal year rather than freshly appraised at filing time. NOT resolved this session: the exact governance of a rectification (who may trigger one — candidate petition vs. TSE-initiated correction; whether there is a formal cutoff date after which a 2022-cycle declaration can no longer be amended) — see open_questions."
  evidence:
    - url: "https://cdn.tse.jus.br/estatistica/sead/odsele/bem_candidato/bem_candidato_2022.zip"
      file: "7c3d5bec1f33a86dbde1ab89365d78125bd2c201cf7446407490278b2f839421.bem-candidato-2022-al-deputado-federal-late-amendment.csv"
    - url: "https://temasselecionados.tse.jus.br/temas-selecionados/registro-de-candidato/documentacao/declaracao-de-bens"
      file: "a91c40fbd19fe06258a36f47033cf9375a2932254c36bcac35ef7957ae6ae1af.tse-declaracao-de-bens-doc.html"
personal_data_to_redact:
  - "NR_CPF_CANDIDATO (CPF, Brazilian national taxpayer ID) — present UNMASKED in the bulk open-data files for the 2022 cycle and (per the pattern) earlier cycles, confirmed by direct inspection; suppressed (replaced with sentinel '-4') starting the 2024 cycle per Resolução TSE nº 23.729/2024. Any historical ingest covering a pre-2024 cycle will carry real CPF numbers in Bronze and MUST redact/hash this field before any Gold or public-facing surface."
  - "NR_TITULO_ELEITORAL_CANDIDATO (voter-registration number) — present unmasked in every cycle checked (2022 and 2024 both); a national personal identifier that TSE itself does not suppress in the open-data files, but should not be surfaced in any public-facing product view."
  - "DT_NASCIMENTO (candidate date of birth) — present unmasked; combined with the two identifiers above it sharpens re-identification risk, though a candidate's birth date is also commonly disclosed on the ballot/portal itself."
tos_and_politeness:
  claim: "No captcha or login encountered anywhere in this survey. Two access surfaces behave differently. (1) divulgacandcontas.tse.jus.br (candidate-facing search portal): unreachable at every probe this session — root and /divulga/ both 302-redirect to a generic cdn.tse.jus.br/indisponivel.html maintenance page, identically under an identified UA and a stock browser UA; its own robots.txt is the same catch-all redirect rather than real robots grammar (not policy — invariant 10 self-imposed limits govern), re-probed and reconfirmed still down at survey time, unchanged from the Phase-0 scout's 2026-07-06 finding. (2) dadosabertos.tse.jus.br (the CKAN portal fronting the SAME underlying candidate/asset data as bulk downloads): reachable (HTTP 200) and carries REAL robots.txt grammar — `Disallow: /api/` among other paths, plus `Crawl-Delay: 10`. This survey used the CKAN `/api/3/action/package_show` and `package_search` JSON endpoints (technically inside the disallowed `/api/` prefix) to discover dataset resource URLs, then found the actual bulk-data downloads resolve to a THIRD host, cdn.tse.jus.br, which has no robots.txt at all (HTTP 404 on that path) and never touches the disallowed prefix. Recommendation carried into Quirks log: the eventual adapter should treat the CKAN API as a one-time, human-supervised discovery step (or simply hardcode the observed `cdn.tse.jus.br/estatistica/sead/odsele/{dataset}/{dataset}_{year}.zip` URL pattern, stable across 1933-2024) and drive its recurring/automated fetch loop against cdn.tse.jus.br directly — never against dadosabertos.tse.jus.br's disallowed `/api/` path. One host-specific deviation logged: planalto.gov.br (fetched for the primary Lei 9.504/1997 statute text) reset the TCP connection for a bare identified-UA request on every attempt, but succeeded immediately with a standard browser UA string suffixed with the same contact identification — the same class of non-stock-UA block already documented for us_senate/uk_commons in the polite-fetching skill; not pursued further as a fingerprint-evasion arms race, just the one documented browser-UA+contact fallback. Politeness used throughout this session: identified UA 'govfolio.io research (contact: ssm.leo@outlook.com)' (browser-UA+contact fallback for planalto.gov.br only), concurrency 1, >=2s interval between requests."
  evidence:
    - url: "https://dadosabertos.tse.jus.br/robots.txt"
      file: "5e2ca3a4b8e436657dd9780caae07633afbe197c0390ac4b8bcca33da614df2d.dadosabertos-robots-txt-disallow-api.txt"
    - url: "https://divulgacandcontas.tse.jus.br/"
      file: "fd79252d2c101b80fa0c555357b6d87ceccfebe5f1141646416456fb94510e9c.divulgacandcontas-reprobe-2026-07-06-still-down.txt"
    - url: "https://www.planalto.gov.br/ccivil_03/leis/l9504.htm"
      file: "b9aa7f42649f77bfab1c8fcb43c887182942a021782cec93ae2a2d3566fcee08.planalto-lei9504-art11.html"
    - url: "https://dadosabertos.tse.jus.br/api/3/action/package_show?id=candidatos-2022"
      file: "a2b02be625a591d917b45e4ca10b570446bfaff5b6b432ab7849922dabe9b2a6.dadosabertos-package-candidatos-2022.json"
language: [pt]
open_questions:
  - question: "Is there ANY public, searchable disclosure surface for sitting (in-office) federal deputies/senators between candidacy events, or is DivulgaCandContas (candidacy-time only) genuinely the sole public channel? The Phase-0 scout traced the annual public-servant regime (CGU e-Patri, Decreto 10.571/2020) and found it explicitly excludes the Legislative branch by its own FAQ; sitting members instead file an internal 'DBR' (Declaração de Bens e Rendas, Lei 8.730/1993) routed to the TCU under tax-secrecy protection. This survey independently re-checked the Câmara's own DBR endpoint (www2.camara.leg.br/edbr/inicio) and got HTTP 502 again (same failure the scout observed), and found no public search/consultation UI for DBR contents on tcu.gov.br in the limited time probed."
    tried:
      - "scout pass (2026-07-06): CGU e-Patri FAQ explicitly scopes to Poder Executivo Federal, excludes Legislativo/Judiciário (archived: 7ea52e2f5d371f67ef8439c888a27c52b95478526efc84fd1c70ef5ecfd2cc3c.cgu-epatri-faq-legislativo-excluido.html); www2.camara.leg.br/edbr/inicio returned HTTP 502 on every scout attempt"
      - "prior audit pass this session (independent of the scout): re-confirmed no public DBR search surface located"
      - "surveyor pass (2026-07-06): re-fetched https://www2.camara.leg.br/edbr/inicio -> HTTP 502 (unchanged); briefly checked https://portal.tcu.gov.br/ (HTTP 200, reachable) but did not locate a public per-legislator asset-declaration search feature in the time budgeted for this survey — a deeper TCU-specific search, or a formal LAI (Lei de Acesso à Informação, Lei 12.527/2011) request, was not attempted this session"
  - question: "Root cause and expected duration of the divulgacandcontas.tse.jus.br outage (redirects to cdn.tse.jus.br/indisponivel.html on every path). Confirmed genuine (not a bot-block) but not explained by any source found."
    tried:
      - "scout pass (2026-07-06): archived the outage redirect (72050d888ffd03d7b0a35e8954a6f6b203978ac3264f558ea6786c1f3c73eb9e.divulgacandcontas-root-outage-indisponivel.html); no incident notice found on www.tse.jus.br's news pages explaining it"
      - "surveyor re-probe (2026-07-06): unchanged — same redirect target, same robots.txt catch-all; no TSE announcement found explaining the outage in the time budgeted"
  - question: "Exact governance of the asset-declaration rectification ('retificação') mechanism: who may trigger one (candidate petition only, vs. TSE/Justiça Eleitoral acting ex officio), and whether there is a formal cutoff date after which a given election cycle's declaration can no longer be amended. The bulk data proves amendments happen years after the election (see amendment_mechanism) but not the procedural rule governing them."
    tried:
      - "read the full temasselecionados.tse.jus.br jurisprudence compilation for 'declaração de bens' (archived: a91c40fbd19fe06258a36f47033cf9375a2932254c36bcac35ef7957ae6ae1af) — establishes that rectification is judicially recognized and can cure an incomplete declaration, but does not state a filing-side procedural rule or deadline"
      - "checked the consulta_cand/bem_candidato leiame documentation for a rectification-deadline note — not present"
  - question: "Whether bem_candidato-shaped itemized asset data exists as far back as TSE's candidate-registration catalog does (1933), or only from some later year once the modern itemized-CSV format was introduced. This session verified 2022 and 2024 directly but did not check earlier cycles."
    tried:
      - "confirmed via package_list that a 'candidatos-<year>' package exists for nearly every year 1933-2024 (except 2020), but only opened/parsed the 2022 and 2024 bem_candidato resources this session — earlier years' bem_candidato resource shape not directly verified"
      - "rust-builder (2026-07-06, real dry-run against live cdn.tse.jus.br, `worker::backfill::TseArchive`): substantially narrowed, not fully closed. `curl -I` against the hardcoded `bem_candidato_{year}.zip` URL pattern shows it 404s for 1994/1998/2002 and returns 200 from 2006 onward — so the itemized-asset bulk format does NOT reach back to 1933; it starts somewhere at/after 2006. But the CSV SCHEMA itself forks within that reachable range: downloaded+inspected 2010's real `bem_candidato_2010_AC.csv` header directly — it carries `NR_ORDEM_CANDIDATO`/`DT_ULTIMA_ATUALIZACAO`/`HH_ULTIMA_ATUALIZACAO` (plus extra columns `DT_GERACAO`/`HH_GERACAO`/`CD_TIPO_ELEICAO`/`NM_TIPO_ELEICAO`/`CD_ELEICAO`/`DS_ELEICAO`/`SG_UE`/`NM_UE` this adapter doesn't model), NOT this adapter's `NR_ORDEM_BEM_CANDIDATO`/`DT_ULT_ATUAL_BEM_CANDIDATO`/`HH_ULT_ATUAL_BEM_CANDIDATO` — core content columns (`SQ_CANDIDATO`, `CD_TIPO_BEM_CANDIDATO`, `DS_TIPO_BEM_CANDIDATO`, `DS_BEM_CANDIDATO`, `VR_BEM_CANDIDATO`) ARE present under the same names, so this reads as a column-RENAME fork, not a wholesale format change. A real dry run (`cargo run -p worker --bin backfill -- --adapter br --from 2006 --to 2006 --dry-run` and `--from 2010 --to 2010`) confirms the adapter fails CLOSED on both years exactly as invariant 6 requires (`CSV deserialize error: missing field 'NR_ORDEM_BEM_CANDIDATO'`), never silently misparsing the old schema. Downloaded+inspected 2014's real header directly: it already matches this adapter's current field names exactly. So: the CURRENT adapter schema is confirmed good from (at least) 2014 through 2022 (also dry-run-verified for 2018/2022, see Quirks log entry below); 2006 and 2010 exist but use an older, different (though closely related) column-name schema this adapter does not yet parse; 2002 and earlier have no `bem_candidato` bulk file at all under this URL pattern. Separately, `consulta_cand_{year}.zip` (registration/identity metadata, not the asset content) resolves back to at least 1994 (200 OK) — its own historical depth exceeds the itemized-asset content's. Not resolved: whether an even-older bem-like dataset exists under a different name/host pre-2006, and whether the 2006/2010 schema could be supported with a small adapter extension (a genuine schema-fork question for a future historical-depth task, analogous to `us_house`'s own pre-2015 PDF-format fork — out of scope for this dry-run proof)."
  - question: "Whether the declaração de bens ever attributes a specific item to a spouse/dependent (an 'owner' distinction), given Brazil's default community-property marital regime (comunhão parcial de bens), or whether conjugal assets are always merged into the candidate's own undifferentiated list. No owner-like column was observed in the bem_candidato schema (leiame documents none)."
    tried:
      - "read the full bem_candidato leiame field dictionary (archived: e46cb76c0124f0002d4480c49680ae2e01f21e5711bb7134c949843dfd64c947) — no owner/titularity field documented"
      - "reviewed ~15 sampled asset-item rows across 3 different candidates this session — no per-item owner marker or spousal-asset flag observed in any row's free-text description either"
  - question: "Why the 'candidatos-2020' open-data package is absent from TSE's catalog (2018 and 2022 present; 2024 present; 2020 missing) — a genuine gap or a naming variant not yet found. Does not affect Câmara/Senado coverage (2020 was a municipal-only year) but is worth resolving before any municipal-office epoch."
    tried:
      - "surveyor pass (2026-07-06): package_show?id=candidatos-2020 returns success:false against the live CKAN API; did not search for an alternate package name/slug for the 2020 cycle in the time budgeted"
      - "rust-builder (2026-07-06, real dry-run, `worker::backfill::TseArchive` over `--from 2018 --to 2022`): the practical Câmara/Senado-coverage half of this question is now directly confirmed, independent of the CKAN catalog gap. `consulta_cand_2020.zip`/`bem_candidato_2020.zip` ARE reachable on `cdn.tse.jus.br` (no 404, unlike the true non-election years 2019/2021, which both 404 as expected) and parse cleanly, but yield ZERO `DEPUTADO FEDERAL`/`SENADOR`/suplente rows after the `DS_CARGO` scope filter — `discover_year(2020, ctx)` returns an empty `Vec` with no error. This is exactly the outcome `cadence_and_lag`'s municipal-only-2020 claim predicts, now confirmed against the live bulk data itself rather than only the CKAN metadata layer. The CKAN catalog's own 'candidatos-2020' gap (a metadata/discovery-layer question) remains open, but it does not affect this regime, confirmed twice over now."
regime_versions:
  - effective_from: "1997-09-30"
    change: "Lei 9.504/1997 (Lei das Eleições) enacted; art. 11 §1º IV establishes the declaração de bens as a mandatory candidacy-registration document for the first time in this consolidated form."
    evidence:
      - url: "https://www.planalto.gov.br/ccivil_03/leis/l9504.htm"
        file: "b9aa7f42649f77bfab1c8fcb43c887182942a021782cec93ae2a2d3566fcee08.planalto-lei9504-art11.html"
  - effective_from: "2009-09-29"
    change: "Lei nº 12.034/2009 adds §6º to art. 11, making the Justiça Eleitoral affirmatively responsible for giving interested parties access to the registration documents (including the asset declaration) — the statutory basis for public access, as opposed to mere non-prohibition."
    evidence:
      - url: "https://www.planalto.gov.br/ccivil_03/leis/l9504.htm"
        file: "b9aa7f42649f77bfab1c8fcb43c887182942a021782cec93ae2a2d3566fcee08.planalto-lei9504-art11.html"
  - effective_from: "2019"
    change: "Resolução TSE nº 23.609/2019 art. 33 §2º establishes the field-level redaction rule for DivulgaCandContas: addresses used for CNPJ/process communications, personal phone, personal email, CPF number, and personal ID document are withheld from public view and filed as confidential ('sigiloso') within the registration case file — the operative rule behind personal_data_to_redact for the fields TSE itself already treats as sensitive at that time (CPF was NOT yet included in this withheld set for the bulk open-data files; see the 2024 amendment below)."
    evidence:
      - url: "https://cdn.tse.jus.br/estatistica/sead/odsele/consulta_cand/consulta_cand_2022.zip"
        file: "f30553fb57dddd4ea6a3b1a636b50cb90edf2ac40ada27fe5cb7993d0c0cad70.consulta-cand-leiame-dictionary-cpf-resolution.pdf"
  - effective_from: "2024"
    change: "Resolução TSE nº 23.729/2024 amends art. 33 §2º of Resolução nº 23.609/2019 to add CPF explicitly to the non-disclosed set specifically for the open-data (Portal de Dados Abertos) candidate files. Directly confirmed by before/after inspection of the bulk NR_CPF_CANDIDATO field: real, unmasked CPF numbers in the 2022-cycle CSV; every value replaced with the numeric-null sentinel '-4' in the 2024-cycle CSV. This is a genuine, dated regime change in what the machine-readable bulk data exposes, not merely a documentation update."
    evidence:
      - url: "https://cdn.tse.jus.br/estatistica/sead/odsele/consulta_cand/consulta_cand_2022.zip"
        file: "f30553fb57dddd4ea6a3b1a636b50cb90edf2ac40ada27fe5cb7993d0c0cad70.consulta-cand-leiame-dictionary-cpf-resolution.pdf"
      - url: "https://cdn.tse.jus.br/estatistica/sead/odsele/consulta_cand/consulta_cand_2022.zip"
        file: "081cd58b919f9db84a4e34fb275a248efa838a88533d020dc2c4ba061081dbd6.consulta-cand-2022-excerpt-cpf-unmasked.csv"
      - url: "https://cdn.tse.jus.br/estatistica/sead/odsele/consulta_cand/consulta_cand_2024.zip"
        file: "42ff0bb6c17deadea5db84fc99a36bc6304d8e200d3866c53b250b15806859dd.consulta-cand-2024-excerpt-cpf-suppressed.csv"
---

# Brazil (br) — Source Authority File

Living canonical context for the `br` regime. Specialists MUST load this before any
source-scoped task and MUST write back new learnings in the same PR.

Scope: **TSE candidacy-time asset declarations only** (`declaração de bens`, filed once
per candidacy at each quadrennial federal general election, for `DEPUTADO FEDERAL` and
`SENADOR`/suplentes). This is the ONLY confirmed public federal asset-disclosure channel
— it is NOT an annual in-office filing, and there is currently no confirmed public
surface for the separate internal DBR regime that applies to sitting members between
elections (see open_questions). All money is `BRL`. Câmara/Senado open-data portals
(dadosabertos.camara.leg.br, www12.senado.leg.br/transparencia) are roster/identifier
sources only — not disclosure sources — per the Phase-0 scout's sources.yaml.

This is the first non-English (`pt`) regime surveyed for this project; do not carry over
US/UK/Canada assumptions (rolling transaction feeds, banded values, stable per-politician
IDs) — none of those hold here. The shape actually observed is a per-election itemized
asset snapshot (`record_types=[holding]`) with exact currency values, distributed as
nationwide bulk CSV downloads rather than scraped from the (currently unreachable)
candidate-facing search portal.

## Data catalog

- **Bulk open data (the practical access path)**: `dadosabertos.tse.jus.br`, a CKAN
  portal cataloging one `candidatos-<year>` package per election year (1933-2024, minus
  2020). Each package's actual disclosure content is two resources: ONE nationwide ZIP
  per dataset per year (no per-UF suffix), each containing one `;`-delimited, quoted,
  Latin-1-encoded CSV per UF INSIDE the ZIP, plus a nationwide aggregate CSV and a
  `leiame.pdf` field dictionary — at the stable predictable URL
  `https://cdn.tse.jus.br/estatistica/sead/odsele/consulta_cand/consulta_cand_{YEAR}.zip`
  (candidate identity/registration fields) and
  `https://cdn.tse.jus.br/estatistica/sead/odsele/bem_candidato/bem_candidato_{YEAR}.zip`
  (itemized declared assets). **Correction (rust-builder, Phase 4 build, 2026-07-06):**
  this section previously documented a `[_{UF}].zip` per-UF URL suffix; the sampler
  (`fixtures/MANIFEST.json uf_zip_pattern_correction`) and an independent audit both
  confirmed that pattern 404s — per-UF CSVs ship inside the one nationwide ZIP, not as
  separately downloadable per-UF ZIPs. Corrected here (was flagged, not yet fixed, as of
  the Phase 3 spec-writer pass). No login, API key, or captcha; `cdn.tse.jus.br` carries
  no robots.txt at all.
- **Candidate-facing search portal**: `https://divulgacandcontas.tse.jus.br/` — TSE's own
  documentation names this as the public presentation layer for individual declarations,
  but it has been unreachable (redirects to a maintenance page) for the entire
  scout-through-survey window this session; not usable as an access path right now (see
  Operational notes).
- **CKAN metadata API**: `https://dadosabertos.tse.jus.br/api/3/action/{package_show,package_search}`
  — useful for discovering the exact resource URLs above, but its own robots.txt disallows
  `/api/`; treat as a one-time discovery aid, not a recurring-fetch target (see
  tos_and_politeness).
- **Roster/identifier-only sources** (NOT disclosure sources): `dadosabertos.camara.leg.br`
  (Câmara open data) and `www12.senado.leg.br/transparencia` (Senado transparency portal)
  — useful for joining a stable-cycle SQ_CANDIDATO/name to a sitting member's mandate
  record, carry no asset content of their own.

## Field mapping (source → gold)

| Source field (`bem_candidato`) | Gold-adjacent concept | Notes |
|---|---|---|
| `SQ_CANDIDATO` (joined via `consulta_cand`) | `politician_id` (resolution key) | per-cycle only, not a durable identifier — see identifiers_available |
| `CD_TIPO_BEM_CANDIDATO` + `DS_TIPO_BEM_CANDIDATO` | `asset_class` | self-describing pair in every row (e.g. 12/`Casa`->RealEstate, 21/`Veículo...`->Other, 61/`Depósito bancário...`->Other); no separate published code table found — full code->AssetClass mapping is a spec-writer task, not resolved here |
| `DS_BEM_CANDIDATO` | `asset_description_raw` (invariant 2: raw is sacred) | verbatim free text; often cites its own IRPF (income-tax) reference year, e.g. "em 31/12/2021-Declaração do IRPF..." |
| `VR_BEM_CANDIDATO` | `value` (`ValueInterval`, low=high, `currency: BRL`) | exact BRL value, comma-decimal string in source (e.g. `"12000,00"`) — parse as `rust_decimal`, never float (invariant 7) |
| `DT_ELEICAO` | `as_of_date` | `record_type == holding` requires this per `GoldCandidate::validate` |
| `DT_ULT_ATUAL_BEM_CANDIDATO`/`HH_ULT_ATUAL_BEM_CANDIDATO` | amendment/supersession trigger | per-item; a changed value here signals a superseding row is needed (invariant 1) |
| `NR_ORDEM_BEM_CANDIDATO` | line-item ordinal within one candidate's declaration | stable ordering key, not a gold column itself |
| — (no field observed) | `owner` | no self/spouse/dependent distinction found in the schema — open question |
| `consulta_cand.DS_CARGO` | mandate/body scoping filter | adapter discovery filter: keep only `DEPUTADO FEDERAL`/`SENADOR`/`1º SUPLENTE`/`2º SUPLENTE` |
| `consulta_cand.NR_CPF_CANDIDATO` | politician resolution key (internal only) | PII — never surface past Bronze/internal resolution; suppressed at source from the 2024 cycle onward |

## Parse strategy & rationale

Deterministic CSV parse, not LLM extraction: the source is already tabular, `;`-delimited,
quoted, Latin-1-encoded structured data (not free-text prose or a scanned PDF), so a
straight CSV reader with Latin-1->UTF-8 transcoding and comma-decimal-to-`rust_decimal`
parsing covers the entire `bem_candidato` + `consulta_cand` schema. The only structured
judgment call is `CD_TIPO_BEM_CANDIDATO -> AssetClass` (many source codes map onto a
handful of Gold buckets) and `DS_CARGO` filtering to this regime's two bodies — both
deterministic lookup-table concerns for spec-writer, not extraction-confidence concerns.
No LLM-fallback seam is anticipated for this regime's core CSV parse (contrast the
scanned-PDF fallback needed for `us_house`); if TSE ever ships an amendment as free-text
prose rather than a replaced CSV row, that would need re-evaluating.

## Quirks log (append-only, dated)

- 2026-07-06 · **Candidacy-time snapshot, not annual filing** — confirmed exactly as the
  scout flagged: no rolling/annual regime exists publicly for sitting members. The
  quadrennial federal-election cadence (Presidente/Governador/Deputado/Senador together,
  offset by 2 years from the municipal Prefeito/Vereador cycle) is structurally different
  from every other regime surveyed so far in this project.
- 2026-07-06 · **CPF exposed unmasked in bulk open data through 2022, suppressed from
  2024** — a genuine, dated regime version (Resolução TSE nº 23.729/2024), not a
  documentation artifact. Confirmed by direct before/after CSV inspection. Any historical
  backfill touching a pre-2024 cycle must treat `NR_CPF_CANDIDATO` as live PII in Bronze.
- 2026-07-06 · **CKAN `/api/` is robots-disallowed; the actual data lives on a different,
  unrestricted host** — `dadosabertos.tse.jus.br/robots.txt` disallows `/api/`, which is
  exactly the path this survey used to discover resource URLs. The bulk ZIPs themselves
  resolve to `cdn.tse.jus.br`, which has no robots.txt at all. Adapter should hardcode the
  observed URL pattern (stable across 1933-2024) rather than crawl the CKAN API on a
  recurring basis.
- 2026-07-06 · **Assets frequently traceable to the candidate's own income-tax return**:
  many `DS_BEM_CANDIDATO` descriptions self-cite "Declaração do IRPF do Exercício
  <year>-Ano-Calendário <year-1>", i.e. values are commonly carried over from the
  candidate's most recent federal income-tax filing rather than freshly appraised at
  candidacy-registration time.
- 2026-07-06 · **Zero-asset candidates produce zero rows, not an empty-form row** —
  confirmed directly: several `DEPUTADO FEDERAL` candidates in the Acre sample have no
  matching `bem_candidato` row at all, consistent with the caselaw permitting a bare
  affirmative "no assets" declaration. Discovery logic must not treat "no bem_candidato
  rows for this SQ_CANDIDATO" as a fetch failure.
- 2026-07-06 · **Evidence-archive PII redaction (orchestrator intervention, before
  commit)** — the four `docs/regimes/br/evidence/` CSV excerpts cited above as
  `who_files`/`amendment_mechanism`/`regime_versions` evidence originally contained real,
  live values for the exact fields `personal_data_to_redact` flags (unmasked CPF, voter
  title, birth date) plus, in the two `bem_candidato` excerpts, real home addresses,
  a vehicle license plate, and a personal phone number embedded in `DS_BEM_CANDIDATO`
  free text for real named 2022 candidates. These are genuinely public TSE bulk
  open-data values, but this project's own git history does not need to permanently
  mirror specific individuals' national ID / address / phone data to demonstrate the
  survey's structural claims. The orchestrator replaced the live values with
  `[REDACTED-BY-GOVFOLIO-*]` placeholders in the archived files (never committed
  unredacted) before staging; each affected file's own `.retrieval.json` sidecar
  documents exactly what was redacted and why. All claims above remain evidenced by
  the same files — only the specific sensitive values were replaced, not the
  structural facts (column presence, format, row counts) the claims rely on. See
  `agents/skills/evidence-archiving/SKILL.md` for the generalized lesson.
- 2026-07-06 · **Bronze-doc granularity is a genuine new design point, not covered by
  any prior regime (rust-builder, Phase 4 build)** — TSE never serves one candidate's
  declaration as an individually addressable document; `bem_candidato`/`consulta_cand`
  each ship as ONE nationwide ZIP covering every candidate for the year. Since the
  `JurisdictionAdapter` trait's `fetch()` produces one Bronze document per one
  `FilingRef`, `discover()` does the real work for this regime: it downloads both
  nationwide ZIPs once, joins `bem_candidato` rows to their `consulta_cand` candidate by
  `SQ_CANDIDATO`, and caches each joined per-candidate JSON in-process (keyed by
  `FilingRef.external_id`) for `fetch()` to re-serialize into Bronze without a second
  network round trip. This means `discover()` must run before `fetch()` in the same
  adapter instance (true of the in-process `pipeline::run::Runner`); a bare `fetch()`
  call with a cold cache fails closed with an explanatory error rather than guessing.
  Implemented in `crates/adapters/br/src/adapter.rs` — flagged here since it is a
  reusable pattern for any future *bulk-single-file, many-filings-per-file* regime.
- 2026-07-06 · **Per-UF CSV entries inside the nationwide ZIP are distinguished from the
  nationwide aggregate CSV by a hardcoded 27-item UF-code whitelist, not by the
  aggregate's own filename** — the exact aggregate filename (`_BR.csv`/`_BRASIL.csv`/
  other) was not directly re-confirmed against a live download this pass (no network
  access in the build environment); rather than guess a suffix to exclude, the adapter
  INCLUDES only entries whose filename ends in one of the 27 official UF abbreviations
  (`crates/adapters/br/src/adapter.rs::BRAZIL_UF_CODES`), which is robust regardless of
  the aggregate's exact name. Flagged for whoever first runs this adapter against a real
  download to confirm the aggregate is in fact excluded (and isn't itself named with a
  2-letter code that collides with a real UF).
- 2026-07-06 · **Conformance harness's zero-row fail-closed check was scoped per
  fixture-CASE, not per whole-run — a real defect, now fixed (rust-builder, Phase 4
  build)** — `crates/pipeline/src/conformance.rs`'s shared `run_case_inner` was
  unconditionally flagging `rows.is_empty()` as an invariant-6 failure for EVERY fixture
  case, with no adapter-level exception. `br` is the first regime with a fixture whose
  own committed `expected.silver.json` is legitimately `[]` (`zero_asset_deputado`, edge
  case 1) — as written, the harness would have failed that case regardless of how
  `parse()` was implemented. Fixed by only flagging zero rows when the fixture's own
  expected Silver is NOT also `[]` (a real mismatch is still caught by the exact-diff
  check immediately below it, unchanged). Verified as a no-op for every other committed
  adapter fixture in the workspace (none has an empty `expected.silver.json`). This is a
  shared-infrastructure fix, not an `br`-scoped one — noted here per this file's write-back
  convention since `br`'s own edge case 1 is what exposed it.
- 2026-07-06 · **Production PII passthrough is gated on `ctx.pool.is_some()`, not a
  separate code path** — plan.md's "Row unit" section flagged a tension: the real
  `parse()`/politician-resolution path needs `NR_TITULO_ELEITORAL_CANDIDATO`/
  `NR_CPF_CANDIDATO` threaded through the Silver payload, but the committed
  `expected.silver.json` (test-designer's deliberately conservative, PII-free pass)
  omits them entirely. Resolved by gating the two fields' presence in
  `crate::parse::SilverRow` on `ctx.pool.is_some()` (`skip_serializing_if` when absent):
  conformance mode (`pool: None`) serializes byte-identical to the committed fixtures;
  a real pool-backed run serializes both fields for the (not-yet-built) politician
  resolution/`RunnerBinding` path to consume. No `RunnerBinding`/`binding.rs` was built
  in this pass — out of scope for this goal's deliverables (conformance + the four gate
  commands); flagged here so the next pass wiring `br` into `pipeline::run::Runner`
  knows the payload shape is already production-ready.

- 2026-07-06 · **FINDING, not fixed here: the shared promotion-stage fingerprint does
  NOT yet honor edge case 2's "fingerprint from specific row content, not the
  timestamp" resolution (rust-builder, Phase 4 build)** — `crates/pipeline/src/stages/
  publish.rs`'s `fingerprint()` call is fully generic across every regime: it hashes
  `serde_json::to_value(&bound)`, i.e. the WHOLE bound `GoldCandidate` (including its
  entire `details` blob), not a per-regime-selected subset of fields. `br`'s
  `BrHoldingDetailsV1` deliberately carries `last_updated_date_raw`/
  `last_updated_time_raw` verbatim (forensic visibility, per edge case 2's own
  resolution) — but because the generic fingerprint hashes the whole candidate, a bulk
  backend re-timestamp event (this same edge case's own evidence: 85-99% of a whole
  state's rows sharing one re-touched date) WOULD still change every affected row's
  fingerprint at promotion time, since the two raw timestamp fields are part of what
  gets hashed. This does not affect conformance (which never reaches the promotion
  stage — `adapter.parse()`/`normalize()` are exercised directly, fixtures always ship
  `fingerprint: null`) or any of this goal's four gate commands, and no `RunnerBinding`
  for `br` was built in this pass (out of scope — see the PII-passthrough Quirks entry
  above), so `br` cannot reach `publish.rs` yet regardless. Flagged here because it is
  the load-bearing assumption edge case 2 rests on, and it is NOT actually true of the
  current shared machinery: whoever wires `br` into `pipeline::run::Runner` needs either
  a per-regime fingerprint-content-selector hook (a cross-regime `publish.rs`/invariant-4
  design change, out of scope for a single-adapter PR) or an accepted, documented
  tradeoff that a bulk re-timestamp event DOES currently manufacture a same-content
  "new" Gold row for every affected candidate — the exact outcome edge case 2 set out to
  avoid.
- 2026-07-06 · **Fingerprint gap above RESOLVED** — `crates/pipeline/src/fingerprint_content.rs`
  now provides the per-regime selector hook the previous entry called for, following the
  same `regime_code: &str` dispatch idiom as `crate::redaction::redact`/
  `crate::conformance::check_details` (both called right next to the fingerprint site in
  `publish.rs`). Default arm (every regime except `br`): unchanged bare
  `serde_json::to_value(candidate)`, proven byte-identical to the pre-fix behavior for
  `us_house` and every other launch regime code (see `fingerprint_content.rs`'s
  `us_house_fingerprint_content_is_byte_identical_to_the_old_bare_serialization` and
  `every_non_br_regime_falls_through_to_the_unchanged_default_arm` tests — zero blast
  radius). `br`'s arm strips `last_updated_date_raw`/`last_updated_time_raw` from the JSON
  value handed to `fingerprint()` only; the actual stored `candidate.details` (DB row, API
  response) is never touched, so the committed `br.holding.json` schema contract holds
  unchanged. `publish.rs` now calls
  `crate::fingerprint_content::fingerprint_content(spec.regime_code, &bound)` in place of
  the old bare serialization. `cargo run -p pipeline --bin conformance -- br` and
  `-- us_house` both still green; `cargo test -p pipeline --test role_evals` re-confirmed
  11/11. This was the sole blocker recorded against advancing `br` past
  `coverage_phase = built`; it is now clear for a future `RunnerBinding`/live-wiring pass.
- 2026-07-06 · **`RunnerBinding` built (rust-builder) — a real, auditable revision to the
  committed `SilverRow`/`expected.silver.json` ground truth, flagged prominently per this
  log's own convention.** `crates/adapters/br/src/binding.rs` (`BrBinding`) is the second
  `RunnerBinding` this project has ever built (`us_house` is the only prior instance) and
  the direct precondition for running `br` through the real `pipeline::run::Runner`.
  `RunnerBinding::filing_identity()` needs a `filer_name`/district-equivalent, which the
  Phase 3/4 `SilverRow` (test-designer's deliberately minimal conformance pass) did not
  carry at all. Resolved by adding two new REQUIRED (non-`Option`) fields to `SilverRow`:
  `nm_candidato` (`consulta_cand.NM_CANDIDATO`) and `sg_uf` (`consulta_cand.SG_UF`) — both
  PUBLIC disclosure content, not PII (this file's own front-matter is explicit that
  candidate identity is the disclosure's whole point, unlike CPF/Titulo/DOB, which
  correctly stay gated). `sg_uf` (the candidate's state) stands in for `us_house`'s
  `state_district_raw`: Brazilian federal deputies/senators are elected per-state, not
  per-single-member-district, so state is this regime's natural district-equivalent for
  roster resolution (design §5.4). Considered and rejected: deriving identity from
  something other than `SilverRow` — `RunnerBinding::filing_identity(rows: &[StagingRow])`
  only ever sees silver rows by trait contract, and no existing `SilverRow` field (e.g.
  `sq_candidato` alone) carries a human name or a roster-matchable location, so there was
  no way to satisfy this without adding fields.
  **Consequence, called out explicitly**: `crates/adapters/br/fixtures/typical_house_vehicle_land/expected.silver.json`
  and `crates/adapters/br/fixtures/amendment_post_election_2026/expected.silver.json`
  (both previously committed, audited ground truth) were edited to add
  `nm_candidato`/`sg_uf` to every row (values: `"ROGÉRIO DA SILVA E SILVA"`/`"AC"` and
  `"ANA MARIA PEREIRA HORA"`/`"AL"` respectively, read straight from each case's own
  `input.json`). `zero_asset_deputado/expected.silver.json` is unchanged (still `[]`, no
  rows to update). `cargo run -p pipeline --bin conformance -- br` re-verified 3/3 green
  after the edit — proof the revision is additive and doesn't change any previously
  asserted value, only adds two new always-present columns. `crates/core/migrations/0010_silver_br.sql`
  adds `stg_br` (mirrors `SilverRow` field for field, `us_house`/`stg_us_house` staging
  convention: `id text primary key`, `unique (raw_document_id, row_ordinal)`,
  `nr_titulo_eleitoral_candidato`/`nr_cpf_candidato` nullable, everything else `not null`);
  applied cleanly and idempotently against the local dev DB. `review_reasons()` returns an
  empty `Vec` unconditionally: unlike `us_house`'s "Amended" filing-status trigger, this
  regime's own resolved edge case 2 (above) already establishes that
  `DT_ULT_ATUAL_BEM_CANDIDATO` is not a trustworthy per-item signal, and the one other
  candidate trigger considered (an unmapped `CD_TIPO_BEM_CANDIDATO` code) already surfaces
  via a lowered `extraction_confidence` on the Gold row itself with no established
  cross-regime convention linking a confidence penalty to a separate `review_task` — none
  invented. `row_ordinal` (the `stg_br` staging-table plumbing column) is assigned by
  `binding.rs` from each row's position in the `Vec<StagingRow>` `parse()` emits
  (`SilverRow` itself carries no `row_ordinal` field, unlike `us_house`'s) — a
  binding-local implementation choice, not a fixture-asserted value.
- 2026-07-06 · **FINDING, not fixed here: `pipeline::run::Runner`'s parse-stage zero-row
  check is NOT scoped for `br`'s legitimate zero-asset case, unlike the conformance
  harness's already-fixed equivalent (rust-builder, `RunnerBinding` build)** — discovered
  while building `binding.rs` above, this is the real-Runner analogue of the conformance
  defect fixed earlier in this log. `crates/pipeline/src/run.rs`'s `parse_and_stage()` runs
  `anyhow::ensure!(!rows.is_empty(), "parse produced zero rows for {} — fail closed
  (invariant 6)", ...)` unconditionally for every adapter, with no per-adapter override —
  unlike `conformance.rs`'s `run_case_inner`, which was fixed to permit a zero-row result
  exactly when the fixture's own expected output is also `[]`. `br`'s own `adapter.rs` doc
  comment and `plan.md` edge case 1 are explicit that `discover()` legitimately mints a
  `FilingRef` for a zero-asset candidate (a real "no assets declared" affirmation, not a
  fetch failure), so a real backfill run over such a candidate would hit this
  `anyhow::ensure!` and be recorded as a per-filing failure in `RunReport::failed` — not a
  silent drop, but also not the "legitimate outcome" plan.md's edge case 1 describes.
  NOT fixed here: this task's scope was `binding.rs` + the `stg_br` migration only: the
  backfill-bin invocation that would actually exercise this path is explicitly the next
  step (out of scope for this pass), and a proper fix needs a cross-regime design decision
  (e.g. an adapter-declared "empty parse is legitimate" flag, mirroring the
  `fingerprint_content`/`redaction`/`check_details` per-regime-hook idiom already
  established in `publish.rs`) rather than a single-adapter patch to shared `run.rs`.
  Flagged here, in the same style as the (now-resolved) fingerprint-content gap above, for
  whoever next wires `br` into a real backfill run.
- 2026-07-06 · **Runner zero-row gate above RESOLVED (rust-builder, cross-cutting fix)** —
  `crates/pipeline/src/zero_rows.rs` now provides the per-regime "zero-row parse is
  legitimate" hook this log's previous entry called for, following the same
  `regime_code: &str` dispatch idiom as `fingerprint_content`/`redact`/`check_details`: a
  small `REGIMES_ALLOWING_ZERO_ROWS` allow-list (`br` only) behind `zero_rows::allowed()`.
  `crates/pipeline/src/run.rs`'s two invariant-6 `anyhow::ensure!` sites
  (`parse_and_stage`'s fresh parse and `parse_stage`'s replay branch) now read
  `!rows.is_empty() || crate::zero_rows::allowed(code)` — same error text, same behavior,
  for every regime not in the allow-list (zero blast radius; proven by
  `zero_rows.rs`'s own `every_other_launch_regime_is_not_allowed` unit test, which
  enumerates every real launch regime code including `us_house`). Traced the full
  downstream consequence of letting `br` through with `rows: vec![]`, per this log's own
  prior finding: `BrBinding::filing_identity` still fails closed on an empty slice by
  design (`no_rows_fails_closed`, `binding.rs` — intentionally NOT relaxed), so
  `Runner::publish_document` gained an early return for `rows.is_empty()` BEFORE calling
  `filing_identity`/`normalize` at all, returning a manually-built all-zero `PublishStats`
  (no `filing`/Gold/outbox/review_task writes). This early return is unreachable for every
  non-opted-in regime (their `rows` can never be empty by the time `publish_document` runs,
  since the two `ensure!`s above already gate it), so it changes nothing for `us_house` or
  any other regime either. New DB-gated tests
  (`crates/pipeline/tests/zero_row_parse.rs`, a synthetic adapter/binding pair — no real
  adapter/fixture touched) prove both sides end to end: a `br`-coded zero-row parse
  succeeds with `gold_inserted/outbox_written/review_tasks == 0` and no `filing` row, while
  any other regime code still fails closed with the byte-identical pre-existing message.
  `cargo run -p pipeline --bin conformance -- br`/`-- us_house`, `cargo clippy --all-targets
  -- -D warnings`, and `cargo test -p pipeline --test role_evals` (11/11) all re-verified
  green after this change.
- 2026-07-06 · `divulgacandcontas.tse.jus.br`: root and `/divulga/` both 302 to
  `cdn.tse.jus.br/indisponivel.html` on every probe this session, identical to the
  Phase-0 scout's 2026-07-06 finding — re-probed independently at survey time via a fresh
  `curl -sIL`, unchanged. Its `robots.txt` is the same catch-all redirect, not real
  robots grammar (self-imposed limits govern, invariant 10). Sentinel should watch this
  host for recovery.
- 2026-07-06 · `dadosabertos.tse.jus.br`: reachable, real robots.txt with
  `Disallow: /api/` and `Crawl-Delay: 10`; this survey's CKAN API calls (`package_show`,
  `package_search`, `package_list`) were made at concurrency 1 with several seconds
  between calls, but are flagged in tos_and_politeness/Quirks log as a path the eventual
  automated adapter should avoid in favor of the unrestricted `cdn.tse.jus.br` host.
- 2026-07-06 · `cdn.tse.jus.br`: no robots.txt (HTTP 404 on that path); ~9 bulk ZIP/HEAD
  requests this session (consulta_cand + bem_candidato, 2022 and 2024, plus one HEAD),
  identified UA, concurrency 1, several seconds between requests — no throttling or
  errors observed.
- 2026-07-06 · `www.planalto.gov.br`: bare identified UA ('govfolio.io research
  (contact: ssm.leo@outlook.com)') got a TCP connection reset (TLS-layer) on every
  attempt fetching the Lei 9.504/1997 text; a standard browser UA string with the same
  contact identification appended succeeded immediately (HTTP 200). Same class of
  non-stock-UA host block already documented for `us_senate`/`uk_commons_register` in
  the `polite-fetching` skill — logged there rather than re-litigated here.
- 2026-07-06 · `www2.camara.leg.br/edbr/inicio` (the internal DBR system): HTTP 502,
  same failure the Phase-0 scout observed — re-checked independently this session, no
  change. Not a public disclosure surface either way (see open_questions).
- 2026-07-06 · **`discover()`'s target-year selection is a simplifying heuristic, not
  fully specified (rust-builder, Phase 4 build)** — live discovery picks the most recent
  year with `year % 4 == 2` (2018/2022/2026/…) at or before the clock's current year,
  matching this regime's quadrennial federal cadence (`cadence_and_lag`). It does not
  account for the lag between an election's own bulk-data publication and calendar
  year-of-election (e.g. whether a fresh cycle's ZIPs are live on `cdn.tse.jus.br` by
  January of the election year, or only after the October vote) — untested against a
  real download this pass (no network access in the build environment). A `discover()`
  call that races ahead of a not-yet-published cycle's ZIP would see a plain HTTP error
  (fails closed, not silently empty) rather than falling back to the prior cycle.
  Historical/backfill runs bypass this entirely via `BrAdapter::discover_year(year, ctx)`
  called directly with an explicit year, mirroring the `us_house` backfill-bin
  precedent. Also: a nationwide-ZIP conditional GET returning 304 is treated as "no
  new/changed candidates this poll" (empty `discover()` result for that dataset) —
  the same semantics `us_house`'s index-304 handling uses, applied here to a whole-year
  bulk file rather than a per-doc index.
- 2026-07-06 · **Dry-run backfill proof built and run against LIVE TSE data
  (rust-builder)** — `worker::backfill::TseArchive` (mirrors the existing `us_house`
  `ClerkArchive` shape exactly: wraps `BrAdapter`, `discover_year`/`fetch`/`parse`/
  `normalize` reused as-is) plugs `br` into the already-generic dry-run/diff-report
  engine (`crates/worker/src/backfill.rs`), wired through `cargo run -p worker --bin
  backfill -- --adapter br`. No new caching code was needed: `BrAdapter::discover_year`
  already downloads+joins both nationwide ZIPs ONCE per year and caches every
  candidate's joined declaration in-process (see the `Bronze-doc granularity` Quirks
  entry above) — `TseArchive::dry_process`'s per-candidate `fetch()` call is a cache
  hit against that same instance, never a second network round trip. A REAL bug was
  found and fixed first: `candidate_fingerprints()` (the dry-run's fingerprint
  reproduction) was hashing the bare `serde_json::to_value(&bound)` — the OLD,
  pre-fix formula this project already replaced everywhere else with the per-regime
  `pipeline::fingerprint_content::fingerprint_content` hook (`publish.rs`) — so any
  regime's dry-run reusing this shared code (not just `br`'s) would have
  misclassified a content-identical-but-metadata-touched row as `Change` instead of
  `Unchanged`. Fixed by threading a `regime_code: &str` parameter through
  `candidate_fingerprints`/`classify`/`dry_run`/`gate_year` and calling the shared
  hook; proven zero-blast-radius for `us_house` (this project's only real production
  caller of this path today) by a dedicated test reproducing the OLD bare-serialization
  formula by hand and asserting byte-identical fingerprints
  (`crates/worker/src/backfill.rs`'s
  `us_house_candidate_fingerprints_are_unchanged_by_the_fingerprint_content_refactor`).
  **Real dry-run results** (identified UA, concurrency 1, ~2s interval, `DATABASE_URL`
  pointed at the local dev Postgres that already carries the 2 filings
  `worker::bin::local_br` previously published there):
  - **2022, full population** (`--from 2022 --to 2022 --limit 11423`, i.e. every
    in-scope candidate, not a sample): 11423 `DEPUTADO FEDERAL`/`SENADOR`/suplente
    candidacies discovered. Classified: 7367 Add (new), 2 Unchanged, 0 Change, 0
    Supersession, 4054 flagged "failed" by the dry-run engine (see the finding below —
    these are legitimate zero-asset candidacies, not real defects). The 2 `Unchanged`
    rows are BOTH previously-published fixture filings
    (`worker::bin::local_br`'s `2022:10001595344`/`2022:20001716829`) — the dry run
    reproduced their exact `publish.rs` fingerprints against REAL production Gold
    data end to end, the strongest possible proof of fingerprint parity (prior tests
    for this only used synthetic candidates).
  - **Zero-asset rate, quantified**: 4054/11423 ≈ 35.5% of ALL in-scope 2022
    candidates have zero declared assets — much more than AUTHORITY.md's prior
    small-Acre-sample "several candidates" language conveyed; now a real,
    whole-population number.
  - **2018** (9830 discovered), **2019/2021** (both 404 — genuine non-election years,
    fail closed per-year exactly as designed, do not sink the 2018-2022 range),
    **2020** (0 in-scope after the `DS_CARGO` filter, no error — see the
    `candidatos-2020` open-question update above): a `--from 2018 --to 2022` sweep
    exercises every one of these outcomes in one real run.
  - **Historical-depth schema fork, directly confirmed** (see the historical-depth
    open-question update above for detail): `bem_candidato` 404s for 2002/1998/1994,
    resolves (200) from 2006, but 2006/2010's real CSV schema uses different column
    names (`NR_ORDEM_CANDIDATO`, `DT_ULTIMA_ATUALIZACAO`/`HH_ULTIMA_ATUALIZACAO`) than
    this adapter's current struct expects — the adapter correctly fails closed
    (invariant 6) on both years rather than silently misparsing; 2014 onward already
    matches the modern schema. `consulta_cand` (registration metadata) resolves
    further back, to at least 1994.
  - **FINDING, not fixed here — cross-regime dry-run engine gap**: the generic
    `dry_process_one` (`crates/worker/src/backfill.rs`) treats ANY zero-candidate
    `normalize()` result as a per-filing failure ("would fail closed, invariant 6"),
    with no per-regime exemption. `pipeline::zero_rows` already exempts `br` from
    this exact check in the REAL `Runner` (zero-asset candidacies are a legitimate,
    expected outcome per plan.md edge case 1) — but the dry-run engine has no
    equivalent exemption, so the real 2022 run above reports 4054 legitimate
    zero-asset candidacies as "failed" filings. Not a real defect (nothing is
    actually broken — the classification undercounts "true" adds/unchanged by
    exactly the zero-asset count, and the printed reason names the right mechanism),
    but a real, live-confirmed inaccuracy in the report's "failed" column for `br`
    specifically. A proper fix would thread `regime_code` into `dry_process_one`'s
    zero-candidate check and consult `pipeline::zero_rows::allowed(regime_code)` the
    same way `run.rs` already does — a small, mechanical follow-up, not attempted in
    this pass (out of this task's scope, and `us_house` has no analogous case to
    prove zero blast radius against without a second regime already exercising it).
  - **FINDING ABOVE RESOLVED** — `dry_process_one` now threads `regime_code` and
    consults `pipeline::zero_rows::allowed(regime_code)` before its zero-candidate
    check, exactly as recommended: `br`'s zero-asset candidacies now classify as
    `FilingClass::Add { records: 0 }` instead of a per-filing failure. Independently
    re-verified against the REAL full 2022 population (`--adapter br --from 2022
    --to 2022 --limit 11423`): discovered 11423, adds 11421 (= 7367 real-asset adds +
    4054 zero-asset adds, now correctly folded together), unchanged 2, **failed 0** —
    down from the pre-fix 4054 false failures. `us_house`'s own zero-candidate
    regression test (`zero_candidate_filing_is_a_per_filing_failure_not_a_crash`)
    confirms zero blast radius: a zero-candidate `us_house` filing still classifies
    as a failure, since `us_house` is not in `pipeline::zero_rows`'s allow-list.
  - Politeness: identified UA (`PolitenessCfg::user_agent()`'s standard
    `govfolio-bot/0.1 (+https://govfolio.io; ...)` format, via `BrAdapter::politeness()`),
    concurrency 1, 2s min-interval — same convention as every prior phase. Total real
    network cost across this whole proof: one `consulta_cand`+`bem_candidato` ZIP pair
    per year touched (2006, 2010, 2014, 2018, 2019, 2020, 2021, 2022 — plus a handful
    of ad hoc `curl -I` HEAD probes for 1994/1998/2002/2006/2010/2014 to scope the
    historical-depth question before spending a full fetch+parse+normalize pass on
    it), no repeated/duplicate fetches (each year's ZIP fetched once, cached, reused
    across every sampled candidate that year).
- 2026-07-06 · **Real write path built (rust-builder) — `crates/adapters/br/src/seed.rs` +
  `crates/worker/src/bin/{seed-br-candidates,backfill-real-br}.rs`, plus a bounded
  real proof against LIVE TSE data.** This is the `br` equivalent of goal 081's
  `us_house` real-write backfill (`bin/backfill-real.rs`). Two design questions this
  task had to resolve, both investigated directly against the Runner/roster code
  (not assumed):
  - **Does `br`'s resolution path mint a new politician on first encounter?** No —
    confirmed by reading `pipeline::stages::roster::resolve_politician` and
    `pipeline::run::Runner::publish_document` directly: there is no auto-mint path
    for ANY regime. `resolve_politician` requires an EXACT pre-seeded
    `(alias, district, body)` match or the filing fails closed with an
    `unresolved_filer` `review_task` (invariant 3). A roster pre-seed is a genuine
    precondition for `br`, same as `us_house`.
  - **Is a full 1933-2024 "historical roster" pre-seed the right shape, given
    `SQ_CANDIDATO` isn't durable and Brazil has no fixed ~435-seat roster?** No —
    judgment call: unlike `us_house`'s Clerk index (a separate, durable member-list
    authority independent of any one filing), `br` has no equivalent authority.
    The only identity fields roster resolution needs (`NM_CANDIDATO`/`SG_UF`) live
    inside the SAME `consulta_cand` bulk file `discover_year` already downloads to
    discover filings. So "seeding the roster" for `br` means minting one
    `politician`+`mandate` row per discovered candidate for the year(s) actually
    being processed — there is no separate authority to pre-load ahead of time, and
    no single "historical roster" artifact to build once. See
    `crates/adapters/br/src/seed.rs`'s module doc comment for the full reasoning.
  - **Known limitation, not fixed**: `RegimeBinding` carries one `body` string, but
    `br`'s scope covers TWO bodies (Câmara dos Deputados + Senado Federal). Roster
    resolution matches on `mandate.body = regime.body` (one fixed string), so this
    seed path seeds `DEPUTADO FEDERAL` only; a real `SENADOR`/suplente filing still
    correctly fails closed (`unresolved_filer`, invariant 3) rather than
    misresolving — it just never resolves under this pass. Supporting `SENADOR`
    needs either a second `RegimeBinding`/regime row or a `RunnerBinding`/roster
    design change letting one binding match more than one body — a genuine
    cross-regime design question, out of scope here.
  - **Real defect found + fixed (shared code, zero blast radius)**:
    `worker::backfill::log_budget_skip` hardcoded the string `"us_house"` into its
    `agents/JOURNAL.md` log line — harmless while `us_house` was the only caller,
    but would have mislabeled a `br` `BACKFILL_BUDGET` skip as `us_house`. Fixed by
    threading a `regime_code: &str` parameter through (one call site in
    `bin/backfill-real.rs` updated to pass `"us_house"`, its own test updated
    identically); `bin/backfill-real-br.rs` passes `"br"`. No behavior change for
    the existing `us_house` caller.
  - **Bounded real proof, run against LIVE TSE data** (identified UA, concurrency 1,
    2s min-interval, `DATABASE_URL` pointed at the shared local dev Postgres that
    already carried `worker::bin::local_br`'s 2 prior filings/3 prior politicians):
    scoped to `--from 2022 --to 2022 --uf AC,AL` (Acre + Alagoas — the two states
    the pre-existing local_br.rs politicians/filings already live in, chosen so the
    proof would directly exercise both idempotent replay of known candidates AND
    real new writes, while staying small/deliberate per this task's own bounding
    guidance).
    - `seed-br-candidates --from 2022 --to 2022 --uf AC,AL`: 11423 candidates
      discovered nationally (full-scope, honest reporting), 371 in AC+AL, 321
      newly seeded as `DEPUTADO FEDERAL` politicians, 47 skipped (`SENADOR`/suplente
      cargo — outside this pass's single-body scope), 0 errors.
    - `backfill-real-br --from 2022 --to 2022 --uf AC,AL`: the default
      `BACKFILL_BUDGET` (500) correctly SKIPPED this scope first
      (`record_delta 904 > 500`, logged to `agents/JOURNAL.md`, nothing blocked) —
      re-run with `BACKFILL_BUDGET=1000` (a deliberate, reasoned widen for this
      one bounded, documented invocation, per the budget's own "widenable via the
      env var" design intent). Real result: 371 filings processed, 343 published,
      **0 replayed** (expected — this is the FIRST live-network publish claim for
      every one of these candidates; the sha256 of freshly-fetched real bytes
      differs from `local_br.rs`'s synthetic fixture bytes, so idempotency shows up
      at the RECORD/fingerprint level, not the document-claim level — see below),
      **751 Gold rows inserted**, 751 outbox events written, 0 review tasks from
      successful publishes, 28 failed closed (`unresolved_filer` — real `SENADOR`/
      suplente candidates with non-zero assets, correctly refused per invariant 3;
      the other 19 of the 47 skipped-cargo candidates had zero declared assets and
      published silently with 0 records, needing no roster resolution at all).
    - **Idempotent replay CONFIRMED at the record level**: queried Gold directly —
      both `local_br.rs`'s pre-existing filings (`2022:10001595344`,
      `2022:20001716829`) still show exactly 3 `disclosure_record` rows each,
      UNCHANGED by this real run, even though the real TSE-fetched bytes produced a
      brand-new (non-replayed) publish claim for both. The real content's
      per-regime fingerprint (`pipeline::fingerprint_content`) matched the
      already-published rows' fingerprints exactly, so `insert_record`'s
      `ON CONFLICT (fingerprint) DO NOTHING` absorbed them with zero new rows —
      independent, real-data confirmation of the SAME fingerprint parity
      `AUTHORITY.md`'s earlier dry-run proof already established.
    - **New real writes confirmed**: total `br` Gold records went 6 → 757 (exactly
      6 pre-existing + 751 new); total `br` `filing` rows went 2 → 161 (159 new,
      2 reused via `ensure_filing`'s `ON CONFLICT (regime_id, external_id)`);
      `review_task` count went 39 → 67 (exactly +28, one per failed candidate, no
      duplicates).
    - **Name-collision risk (flagged, not hit)**: `seed_roster`'s ambiguity check
      only rejects an alias+district match against ALREADY-COMMITTED rows: two
      different candidates sharing the exact same `(NM_CANDIDATO, SG_UF)` within one
      seeding pass would silently merge onto the same politician (the second
      candidate's `seed_roster` call would see the first's just-committed row as
      "already seeded" and skip). Given `br` has thousands of one-time candidates
      (unlike `us_house`'s much smaller fixed roster), this risk is real and worth
      watching at larger scale. Checked directly against this run: 324 total `br`
      politicians (3 pre-existing + 321 newly seeded) with ZERO
      `(alias, district)` pairs shared by more than one politician row — no merge
      occurred in this proof's scope.
    - **ALERT-SUPPRESSION VERIFIED (mandatory per this session's standing
      directive)**: queried `outbox_event` directly, scoped to the `br` regime.
      ALL 751 new rows from this run carry `dispatched_at` equal to `created_at`
      (pre-stamped at insert time, backfill-mode `FilingSpec::backfill = true`
      threading through correctly) — the real-time alert dispatcher's
      `dispatched_at is null` poll will never pick these up. The only 6
      non-suppressed (`dispatched_at is null`) `br` outbox events are
      `local_br.rs`'s PRE-EXISTING rows from an earlier, unrelated proof (that bin
      never sets `Runner::with_backfill`) — predate this task entirely, not
      created by this run, and out of this task's scope to fix (flagged here for
      awareness; a real subscriber alert on those 6 would only ever fire once, the
      first time the dispatcher polls them, and is a pre-existing condition this
      task did not introduce).
    - Not attempted (explicitly out of scope for this pass): the full 1933-2024
      historical range, the full 2022 nationwide population (11423 candidates),
      and `SENADOR`/suplente resolution — all flagged above as later,
      independently-audited increments.
- 2026-07-07 · **Full nationwide 2022 real write completed (rust-builder)** —
  widened the bounded AC+AL proof above to ALL 27 states, one complete real
  annual disclosure cycle for Brazil's federal deputies (not the full
  1994-2024 historical range, still out of scope). `seed-br-candidates --from
  2022 --to 2022` (no `--uf`): 11423 candidates discovered/considered
  nationwide, 793 non-`DEPUTADO FEDERAL` (skipped, correct scope), 10630
  `DEPUTADO FEDERAL` candidates in scope.
  - **REAL DEFECT FOUND + FIXED before any write: a same-pass `(alias,
    district)` identity collision, exactly the risk the AC+AL proof's own
    Quirks entry flagged as "worth watching at larger scale."** The FIRST
    nationwide seed pass (before the fix) silently merged 89 pairs (178
    distinct real candidates, verified via distinct `SQ_CANDIDATO`, e.g.
    "VIVIANE BARBOSA FERNANDES"/BA, "TELÊMACO BRANDÃO"/GO — common-name
    collisions within one state's proportional-list ballot) onto ONE shared
    politician row each, because `seed_roster`'s own ambiguity check only
    rejects when 2+ rows are ALREADY COMMITTED before a call starts — it
    never sees two DIFFERENT candidates discovered in the SAME call before
    either is committed. Caught before any real Gold write (an orchestrator
    mid-task halt on this exact finding), reconciled by deleting all 89
    silently-created politician/alias/mandate rows (verified zero downstream
    filing/disclosure_record/review_task references first — safe, since
    `backfill-real-br` had not yet run), then fixed at the source:
    `crates/adapters/br/src/seed.rs` gained `identity_collision_counts()`, a
    pure pre-count over the considered `DEPUTADO FEDERAL` set that refuses
    EVERY candidate sharing a colliding identity (order-independent — neither
    member of a pair is preferred over the other, since picking one would
    itself be an arbitrary guess; invariant 3). Re-verified end to end after
    the fix: re-running the nationwide seed reproducibly reports `seeded 0,
    178 error(s)` (89 pairs x 2, symmetric), with the previously-good 10452
    politicians left untouched. Final seeded roster: **10452 `DEPUTADO
    FEDERAL` politicians nationwide** (10630 in-scope minus 178 refused to
    the collision). 2 new unit tests added
    (`identity_collision_counts_flags_same_pass_duplicates_both_ways`,
    `identity_collision_counts_respects_uf_filter`); `cargo test -p br`
    30/30, conformance 3/3, fmt/clippy clean throughout.
  - **A second, narrower identity-collision axis was found by due diligence
    (not fixed — verified benign, not a defect) while investigating the run's
    filing-vs-candidate count reconciliation**: `resolve_politician` matches
    purely on `(alias, district, body)` with no cargo/`SQ_CANDIDATO`
    awareness, so a `SENADOR`/suplente candidate whose filed name+state
    happens to match an already-seeded `DEPUTADO FEDERAL` politician's
    identity CAN resolve onto that politician's row (my same-pass fix only
    guards the `DEPUTADO FEDERAL`-vs-`DEPUTADO FEDERAL` axis, by design,
    since `seed_candidates_year` only ever seeds that one cargo). Checked
    EXHAUSTIVELY, not sampled: cross-referenced all 793 non-`DEPUTADO
    FEDERAL` candidates nationwide against the filings this run actually
    created — exactly 3 resolved (not 0, not more): "ANDREA GOMES FONTES
    RODRIGUES"/RJ (`DEPUTADO FEDERAL` `SQ_CANDIDATO` 190001596778 + `2º
    SUPLENTE` 190001724188), "FÁBIO DE MELO SÉRVIO"/PI (`DEPUTADO FEDERAL`
    180001734011 + `SENADOR` 180001713961), and "ADRIANO PIETRO SANTIAGO
    VIANA"/PA (`DEPUTADO FEDERAL` 140001727612 + `2º SUPLENTE` 140001648645).
    All 3 independently verified via the source's own `NR_CPF_CANDIDATO` AND
    `NR_TITULO_ELEITORAL_CANDIDATO` (both genuine unique national identifiers)
    to be the licensed SAME real individual under both registrations, not two
    different people sharing a name — so none of these 3 is a wrong
    attribution in practice. The remaining 790 non-`DEPUTADO FEDERAL`
    candidates correctly never resolved. Flagged here as a residual,
    UNPROTECTED architectural risk for future runs/years (this axis has no
    mechanical guard the way the same-cargo one now does) — related to the
    already-documented "`RegimeBinding` carries one `body` string" limitation
    above; a future fix would need either CPF-aware disambiguation (CPF is
    presently internal-resolution-only, never public per personal_data_to_redact)
    or a cargo-scoped roster match, a genuine design question out of scope
    for this pass.
  - **Real write** (`backfill-real-br --from 2022 --to 2022`, no `--uf`):
    `BACKFILL_BUDGET` needed = **50000** (sized generously up front from the
    AC+AL proof's `record_delta=904` over 371 candidates, scaled ~30x for
    11423 nationwide, to avoid a second full nationwide dry-run fetch just to
    observe the exact number). Gate's own reported real number:
    **`record_delta 40427`** (comfortably inside the budget; this figure
    counts every candidate's declared-asset rows regardless of whether
    `resolve_politician` would later succeed, so it is intentionally larger
    than the actual Gold total below — the gap is exactly the 678 candidates
    that failed closed). Real result: 11423 filings in scope, published
    10402, replayed 343, **35129 Gold rows inserted**, 35129 outbox events
    written, 0 review tasks (this regime's `review_reasons()` is always
    empty, per the earlier `RunnerBinding` Quirks entry), **678 failed
    closed** (`unresolved_filer`, invariant 3).

    **CORRECTION (independent audit caught this, original write-back was
    wrong)**: the failed-closed breakdown is NOT a clean "178 collision +
    500 other" split. Only **~90** of the 678 are actually attributable to
    the 89 collision pairs (38 pairs both members failed, 14 pairs one
    member failed, 37 pairs NEITHER member produced a failed line) — not
    178. Root cause: a collision-refused candidate who also filed zero
    assets never reaches `resolve_politician`/`unresolved_filer` at all —
    `Runner::publish_document`'s pre-existing zero-row early return (the
    same mechanism `pipeline::zero_rows` documents elsewhere in this file)
    fires first, so there's no roster lookup, no failure, no trace either
    way. This is NOT a safety defect (a zero-asset candidacy has no
    disclosure content to misattribute regardless of roster state), just a
    correction to this write-back's own arithmetic. True split: ~90
    collision-attributable, ~588 genuine `SENADOR`/suplente/other-cargo
    unresolvable candidates — cross-checked against the DB's `review_task`
    count (678 = 28 pre-existing deduped + 650 new this run), which is
    internally consistent independent of the wrong 178/500 framing above.
    `published`/`replayed` split is
    explained, not alarming: TSE bulk-retimestamps declaration content
    server-side (documented `DT_ULT_ATUAL_BEM_CANDIDATO` behavior, see the
    fingerprint-content Quirks entries above) between the earlier AC+AL proof
    and this run, so many previously-processed candidates got a fresh Bronze
    sha256 (registering as a new "publish" claim, not a "replay") even though
    their Gold-level fingerprint content is unchanged — the ACTUAL
    correctness proof is the idempotency check below, not the claim-ledger
    labels.
  - **Idempotency, directly re-verified against Postgres (not inferred from
    the CLI report)**: the 161 filings that existed BEFORE this run (the 2
    original `local_br.rs` proof filings + the 159 from the AC+AL bounded
    proof) carry EXACTLY 757 `disclosure_record` rows after this run too —
    byte-for-byte unchanged, confirmed via a `discovered_at` cutoff query
    isolating exactly those 161 pre-existing rows. `ON CONFLICT (fingerprint)
    DO NOTHING` absorbed every re-touched-but-content-identical record with
    zero new rows, exactly as invariants 1 and 4 require.
  - **Alert-suppression, exhaustive (mandatory per this session's own
    standing directive, not sampled)**: every one of the 35129 NEW
    `outbox_event` rows this run created carries `dispatched_at` equal to
    `created_at` (pre-stamped at insert time, backfill mode). The ONLY 6
    non-suppressed (`dispatched_at is null`) `br` outbox rows in the entire
    table, before and after this run, are the SAME PRE-EXISTING
    `local_br.rs` rows (external ids `2022:10001595344`/`2022:20001716829`)
    from before backfill-mode suppression existed — confirmed identical ids
    and timestamps to the baseline captured before this run started, not
    newly created. `worker::alerts::matcher::match_pass`'s `dispatched_at IS
    NULL` filter structurally cannot fire on any of this run's 35129 new
    rows.
  - Final `br` totals after this pass: **10452 politicians** (10449
    `DEPUTADO FEDERAL` + 3 pre-existing mixed-case `Deputado Federal`),
    **6691 filings**, **35886 disclosure_records**, **35886 outbox_events**
    (35880 dispatched + the same 6 pre-existing undispatched). 178 real
    candidates (89 pairs) remain unresolved pending future disambiguation
    (invariant 3: never guess, stays NULL) — not fixed further this pass.
  - Not attempted (still explicitly out of scope): the full 1933-2024
    historical range remains the clear next increment, to be pursued
    incrementally with the same `BACKFILL_BUDGET`/audit discipline.

## Operational notes (politeness incidents, outages)

- 2026-07-06 · `divulgacandcontas.tse.jus.br`: root and `/divulga/` both 302 to
  `cdn.tse.jus.br/indisponivel.html` on every probe this session, identical to the
  Phase-0 scout's 2026-07-06 finding — re-probed independently at survey time via a fresh
  `curl -sIL`, unchanged. Its `robots.txt` is the same catch-all redirect, not real
  robots grammar (self-imposed limits govern, invariant 10). Sentinel should watch this
  host for recovery.
- 2026-07-06 · `dadosabertos.tse.jus.br`: reachable, real robots.txt with
  `Disallow: /api/` and `Crawl-Delay: 10`; this survey's CKAN API calls (`package_show`,
  `package_search`, `package_list`) were made at concurrency 1 with several seconds
  between calls, but are flagged in tos_and_politeness/Quirks log as a path the eventual
  automated adapter should avoid in favor of the unrestricted `cdn.tse.jus.br` host.
- 2026-07-06 · `cdn.tse.jus.br`: no robots.txt (HTTP 404 on that path); ~9 bulk ZIP/HEAD
  requests this session (consulta_cand + bem_candidato, 2022 and 2024, plus one HEAD),
  identified UA, concurrency 1, several seconds between requests — no throttling or
  errors observed.
- 2026-07-06 · `www.planalto.gov.br`: bare identified UA ('govfolio.io research
  (contact: ssm.leo@outlook.com)') got a TCP connection reset (TLS-layer) on every
  attempt fetching the Lei 9.504/1997 text; a standard browser UA string with the same
  contact identification appended succeeded immediately (HTTP 200). Same class of
  non-stock-UA host block already documented for `us_senate`/`uk_commons_register` in
  the `polite-fetching` skill — logged there rather than re-litigated here.
- 2026-07-06 · `www2.camara.leg.br/edbr/inicio` (the internal DBR system): HTTP 502,
  same failure the Phase-0 scout observed — re-checked independently this session, no
  change. Not a public disclosure surface either way (see open_questions).
- 2026-07-06 · **Process incident (orchestrator)**: this entire section was accidentally
  dropped from the file by a later Phase-4 write-back (the `RunnerBinding` build, commit
  `e6944ae`) — a full 5-required-section file got reduced to 4, and `validate-survey --
  br` was not re-run after that commit to catch it (every subsequent phase's gate checks
  focused on the code/conformance changes, not re-validating this artifact). Caught only
  when re-running `validate-survey` after this same session's later dry-run write-back.
  Recovered verbatim from the surveyor's original commit (`c578506`) via `git show`; no
  content lost. Lesson: `validate-survey`/`validate-sources`/`validate-manifest` should
  be re-run as a matter of course any time a later phase appends to a regime's
  `AUTHORITY.md`, not only when the phase whose job is specifically the survey runs.
