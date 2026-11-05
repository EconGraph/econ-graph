# Draft FRED discovery coverage for PR #722

**DRAFT — NOT READY TO DEPLOY.** This proposes all 33 historical subject families through automatic catalog predicates. It does not establish complete current API membership or operational coverage. No owner API key was accessed.

## Bounds and acceptance

`max_series = 500` and `max_catalog_series = 20000` are **deliberate blocking placeholder bounds**, inherited from the GDP-only configuration. A 500-series discovery cannot fulfill the historical 1,533-ID target. Neither value is an accepted operating budget. Retain the complete proposed scope, measure it, then choose bounds that fit the user goal with reviewed headroom. Do not drop families to fit these placeholders.

The draft contains **83 rules across 71 releases** and uses the bounded 128-rule parser ceiling; the former 64-rule ceiling refused it. The existing 10,000 selected-series and 100,000 scanned-row hard ceilings are independent constraints and may also need an explicit decision after measurement. Parsing and witness matches are not readiness evidence.

Rule keys are comments before `[[rules]]`, not new TOML fields: `key` and `name` would violate the parser's unknown-field policy. Fixture indices are zero-based. The grouped walk must inspect each catalog once, evaluate all its predicates and deduplicate selected IDs while retaining per-family counts.

## All 33 families

The historical counts describe the old PR, not current availability. Each example is one membership witness, not the complete family. The fixture records the exact official title/frequency field provenance and page hash. Arbitrary frequency words elsewhere on a page are not metadata. USREC uses its existing repository fixture.

| Family | Historical IDs | Rule keys / small overrides | Representative witness |
| --- | ---: | --- | --- |
| 01. GDP and national accounts | 22 | `national-accounts`, `personal-income-outlays`, `potential-output-unemployment` | [GDPPOT](https://fred.stlouisfed.org/series/GDPPOT) |
| 02. NIPA and PCE detail: GDP components, income, profits, PCE by type and prices | 119 | `national-accounts`, `personal-income-outlays`, `trimmed-pce`; overrides: `GDPNOW`, `STLENI` | [DPCERL1Q225SBEA](https://fred.stlouisfed.org/series/DPCERL1Q225SBEA) |
| 03. Prices: CPI, PCE, PPI | 16 | `personal-income-outlays`, `national-cpi`, `ppi-headlines`, `interest-rate-spreads`; overrides: `CPIAUCSL` | [T10YIE](https://fred.stlouisfed.org/series/T10YIE) |
| 04. Price detail: CPI components, expectations, PPI, trade, energy, average prices | 108 | `national-cpi`, `ppi-headlines`, `ppi-demand`, `ppi-commodity-sectors`, `ppi-industry-sectors`, `import-export-price-aggregates`, `energy-spot-prices`, `retail-fuel-prices`, `national-average-prices`, `expected-inflation`, `sticky-flexible-cpi`, `median-trimmed-cpi`, `interest-rate-spreads` | [EXPINF1YR](https://fred.stlouisfed.org/series/EXPINF1YR) |
| 05. Employment and labor | 25 | `potential-output-unemployment`, `national-employment`, `unemployment-claims`, `jolts`; overrides: `UNRATE`, `PAYEMS` | [ICSA](https://fred.stlouisfed.org/series/ICSA) |
| 06. Employment detail: industries, earnings, demographics, JOLTS, productivity | 191 | `national-employment`, `unemployment-claims`, `jolts`, `productivity-costs`, `employment-cost-index`, `weekly-earnings`, `wage-growth` | [ECIWAG](https://fred.stlouisfed.org/series/ECIWAG) |
| 07. Interest rates, monetary policy, Treasury yields | 23 | `selected-interest-rates`, `interest-rate-spreads`, `policy-target-range`, `effective-federal-funds`; overrides: `FEDFUNDS`, `DGS10` | [EFFR](https://fred.stlouisfed.org/series/EFFR) |
| 08. Rates and spreads detail: Treasury, TIPS, bills, paper, money markets, loans | 77 | `selected-interest-rates`, `interest-rate-spreads`, `effective-federal-funds`, `commercial-paper`, `sofr`, `sofr-averages-index`, `overnight-bank-funding`, `reserve-balance-interest`, `temporary-open-market`, `consumer-credit` | [SOFR](https://fred.stlouisfed.org/series/SOFR) |
| 09. Financial conditions and activity indexes | 16 | No primary rule; overrides: `STLFSI4`, `NFCI`, `ANFCI`, `NFCIRISK`, `NFCICREDIT`, `NFCILEVERAGE`, `NFCINONFINLEVERAGE`, `KCFSI`, `CFNAI`, `CFNAIMA3`, `CFNAIDIFF`, `WEI`, `RECPROUSM156N`, `SAHMREALTIME`, `SAHMCURRENT`, `JHGDPBRINDX` | [STLFSI4](https://fred.stlouisfed.org/series/STLFSI4) |
| 10. Money supply and Federal Reserve balance sheet | 6 | `money-stock-reserves`, `fed-balance-sheet`, `money-velocity`; overrides: `M2SL` | [M2V](https://fred.stlouisfed.org/series/M2V) |
| 11. Money and Federal Reserve balance sheet detail | 19 | `money-stock-reserves`, `fed-balance-sheet`, `money-velocity`, `real-money-stock` | [M2REAL](https://fred.stlouisfed.org/series/M2REAL) |
| 12. Credit: lending standards, delinquencies, bank and household credit | 51 | `lending-standards`, `delinquencies-chargeoffs`, `consumer-credit`, `all-commercial-bank-credit`, `household-debt-service`, `financial-account-concepts` | [TNWBSHNO](https://fred.stlouisfed.org/series/TNWBSHNO) |
| 13. Housing | 8 | `residential-construction`, `new-residential-sales`, `vacancy-homeownership`, `national-house-prices`; overrides: `HOUST` | [RRVRUSQ156N](https://fred.stlouisfed.org/series/RRVRUSQ156N) |
| 14. Housing detail: starts, permits, sales, prices, vacancy, construction spending | 53 | `residential-construction`, `new-residential-sales`, `vacancy-homeownership`, `construction-spending`, `national-house-prices` | [NHFSEPCS](https://fred.stlouisfed.org/series/NHFSEPCS) |
| 15. Trade and balance of payments | 6 | `national-accounts`, `international-trade`, `international-transactions` | [IEABC](https://fred.stlouisfed.org/series/IEABC) |
| 16. International headline: exchange rates, dollar, trade partners, World Bank GDP | 123 | `national-accounts`, `international-trade`, `international-transactions`, `daily-exchange-rates`, `daily-nominal-dollar`, `monthly-exchange-rates`, `monthly-nominal-dollar`, `real-dollar`, `wdi-major-country-gdp`, `wdi-real-gdp-per-capita`, `wdi-inflation`; overrides: `NYGDPMKTPCDWLD` | [MKTGDPCNA646NWDB](https://fred.stlouisfed.org/series/MKTGDPCNA646NWDB) |
| 17. Debt and fiscal | 5 | `treasury-debt-holdings`, `debt-fiscal-gdp-ratios`, `annual-fiscal-budget` | [GFDEBTN](https://fred.stlouisfed.org/series/GFDEBTN) |
| 18. Fiscal detail | 10 | `treasury-debt-holdings`, `debt-fiscal-gdp-ratios`, `annual-fiscal-budget`, `monthly-treasury` | [MTSDS133FMS](https://fred.stlouisfed.org/series/MTSDS133FMS) |
| 19. Industrial production and manufacturing | 8 | `industrial-production-capacity`, `manufacturers-orders-inventories`, `manufacturing-trade-sales`, `advance-retail`, `retail-sector-detail`; overrides: `INDPRO` | [DGORDER](https://fred.stlouisfed.org/series/DGORDER) |
| 20. Production, orders, inventories, sales and vehicles | 72 | `industrial-production-capacity`, `manufacturers-orders-inventories`, `manufacturing-trade-sales`, `advance-retail`, `retail-sector-detail`, `real-retail`, `retail-ecommerce`, `motor-vehicles`, `transportation-services` | [TOTALSA](https://fred.stlouisfed.org/series/TOTALSA) |
| 21. Business cycle | 1 | No primary rule; overrides: `USREC` | `USREC` (existing fixture) |
| 22. Regional headline: state unemployment rate (seasonally adjusted), one per state and DC | 51 | `state-unemployment-sa` | [CAUR](https://fred.stlouisfed.org/series/CAUR) |
| 23. Regional: unemployment rate, not seasonally adjusted, by state and DC | 51 | `state-unemployment-nsa` | [CAURN](https://fred.stlouisfed.org/series/CAURN) |
| 24. Regional: labor force participation rate, by state and DC | 51 | `state-participation-sa` | [LBSSA06](https://fred.stlouisfed.org/series/LBSSA06) |
| 25. Regional: all employees, total nonfarm, by state and DC | 51 | `state-nonfarm-employment` | [CANA](https://fred.stlouisfed.org/series/CANA) |
| 26. Regional: all employees, manufacturing, by state and DC | 49 | `state-manufacturing-employment` | [CAMFG](https://fred.stlouisfed.org/series/CAMFG) |
| 27. Regional: real GDP (quarterly), by state and DC | 51 | `state-real-gdp` | [CARQGSP](https://fred.stlouisfed.org/series/CARQGSP) |
| 28. Regional: per capita personal income (annual), by state and DC | 51 | `state-per-capita-income` | [CAPCPI](https://fred.stlouisfed.org/series/CAPCPI) |
| 29. Regional: real median household income (annual), by state and DC | 51 | `state-real-household-income` | [MEHOINUSCAA672N](https://fred.stlouisfed.org/series/MEHOINUSCAA672N) |
| 30. Regional: resident population (annual), by state and DC | 51 | `state-population` | [CAPOP](https://fred.stlouisfed.org/series/CAPOP) |
| 31. Regional: FHFA all-transactions house price index (quarterly), by state and DC | 51 | `state-all-transactions-hpi` | [CASTHPI](https://fred.stlouisfed.org/series/CASTHPI) |
| 32. Regional: new private housing units authorized by building permits, by state and DC | 51 | `state-building-permits` | [CABPPRIVSA](https://fred.stlouisfed.org/series/CABPPRIVSA) |
| 33. Regional: CPI (all items, NSA) for census regions and metro areas | 15 | `regional-all-items-cpi` | [CUUR0100SA0](https://fred.stlouisfed.org/series/CUUR0100SA0) |

The old 1,533-ID list is a one-time historical audit input, not runtime membership. The new fixture keeps one representative per family and geography negatives. It does not certify every old ID's availability, ownership, restriction status or selection. The final API report must classify every mismatch rather than silently excluding it.

## Scope rationale and unresolved breadth

- National accounts uses releases 53/54 plus potential output/unemployment 99. The GDP-title-only predicate omitted components, income and PCE detail. GDPNow and STLENI remain small headline exceptions.
- National employment includes household/establishment detail, claims, JOLTS, productivity, ECI, earnings and wage growth. The proposed whole-release employment/productivity detail is broader automatic membership, not an accepted first-crawl budget. A CES-only or LNS-only rule is insufficient.
- CPI national products use `0000` geography codes and headline aliases; regional all-items is separate. PPI demand/commodity/industry prefixes are inferred from the old examples. The current taxonomy and all additional matches still need API verification; a few examples do not prove the entire price family.
- Credit separates standards, delinquencies, consumer credit, H.8, debt service and Z.1 concepts. Institution variants are needed for historical DRCLACBN. The H.8 proposal selects all-commercial-bank aggregates: specifically verify H8B3092NCBA instead of asserting that it matches.
- Daily FX is release 17; monthly FX and nominal monthly dollar indexes are 15; real dollar indexes are 245. SOFR averages/index is 483, independently of daily SOFR 445.
- WDI keeps the historical per-indicator geography vocabularies: twenty nominal-GDP country codes, six real-per-capita countries plus world, and ten inflation countries plus world. These are automatic indicator/product patterns, not 1,533 copied members. A wider country/aggregate scope is an explicit owner decision.
- Fiscal and production/order/retail/vehicle/transport subfamilies have separate rules. Unavailable fiscal aliases such as FYGFDPUN require an explained API result; a release label does not prove an old identifier still exists.

## Regional isolation

Patterns match entire identifiers. `STATE` is the fifty-state-plus-DC vocabulary; participation uses their FIPS codes. Counties, metros, territories, BEA regions, industry detail and other products must not enter a state family merely because titles resemble one another.

| Family | Release | Shape and frequency |
| --- | ---: | --- |
| Unemployment SA / NSA | 112 | `STATE + UR` / `STATE + URN`, monthly |
| Participation SA | 446 | `LBSSA + state FIPS`, monthly |
| Nonfarm / manufacturing employment | 112 | `STATE + NA` / `STATE + MFG`, monthly |
| Real GDP | 140 | `STATE + RQGSP`, quarterly |
| Per-capita income | 110 | `STATE + PCPI`, annual |
| Real household income | 249 | `MEHOINUS + STATE + A672N`, annual |
| Population | 118 | `STATE + POP`, annual |
| All-transactions HPI | 171 | `STATE + STHPI`, quarterly |
| Permits | 148 | `STATE + BPPRIVSA`, monthly |
| Regional CPI | 10 | `CUUR` region/metro area code plus `SA0`; retain published frequency |

Monthly CANA/CAMFG belong to release 112; release 308 annual variants are not a faithful replacement. The old manufacturing family has 49 members: do not invent two missing upstream products. The regional CPI proposal follows matching area codes automatically; choosing exactly the historical fifteen versus all current matching areas remains an owner decision.

## Small overrides

The 30 includes preserve the existing nine headlines and two known-retired probes, add sixteen financial/activity indicators, two nowcasts and one world GDP aggregate. Family 09 is deliberately a small headline family. These exceptions do not replace primary automatic membership for the other families. Only the two existing retired probes are `expected_missing`; no new miss is declared expected without upstream evidence.

## Evidence limits and leaf scopes

The [fixture](../../backend/crates/econ-graph-crawler/tests/fixtures/fred/coverage-family-witnesses.json) contains provenance and public counts. Completed browsable listings span requests and are not pinned API inventories. **Every listing ending at page 30 is incomplete and supplies no upper bound**, including the large state GDP, state employment, permits, HPI and WDI catalogs.

- Public release 17 contained 33 IDs, including 23 DEX matches; release 15 contained 31, including 24 EX matches.
- Participation 446 contained 102 IDs, including 51 SA matches; population 118 contained 73, including 51 state/DC matches.
- Other complete browsable counts: construction 27 = 121; income/outlays 54 = 144; consumer credit 14 = 159; new residential sales 97 = 386; lending survey 191 = 639; JOLTS 192 = 540. These whole-listing counts are not necessarily the predicate subsets or API counts.
- Housing leaves 32300–32304 contain 8/8/26/8/26 browsable IDs. They do not establish all original regional construction coverage; draft release 27 is itself a small bounded public listing. Category 97 (Housing) and release 97 (New Residential Sales) are different scopes.
- Nominal-GDP witness metadata links GDP category 32291. The per-capita and inflation witnesses do not establish membership in that leaf. No verified common WDI leaf covers all three indicator patterns. The draft retains 234 with an explicit unknown scan-cost blocker rather than inventing a smaller replacement.

## Required validation before acceptance

1. Use an owner-provided API environment through the existing read-only checker. Never commit keys or request URLs containing them. The documentation's demonstration API key is not a validation environment.
2. Pin the realtime date and retrieve all pages with count, offset, order, duplicate and truncation checks. Record unique scanned rows per physical catalog, all per-family matches, selected union, restrictions and expected retirement notices.
3. Audit the complete historical input against that actual union. Explain source ownership, changed aliases/taxonomies, geography, restriction and availability gaps. Do not turn unexplained misses into exclusions.
4. Agree national detail breadth, international geography growth, regional CPI areas and initial full-vintage workload. Set global and per-family bounds from the full accepted scope, with explicit headroom. The present 500/20,000 bounds remain blocking placeholders.
5. If full grouped scans exceed the 100,000 hard ceiling, verify leaf scopes or design documented server-side tag/frequency filtering with correct grouping and lifecycle-scope identity. Client-side ID predicates do not reduce scan cost.
6. Verify the generic adapter contracts, actual-catalog bindings for every family, geography witnesses, overlap/grouping behavior, restrictions and volume guards. Verify single-job/chunked equivalence and lifecycle behavior with a database configured. Record revision-specific local and hosted results in the PR, distinguishing database early returns and skipped cases from executed assertions. These checks do not establish live API counts, complete historical membership or workload cost.

Official sources: [release API](https://fred.stlouisfed.org/docs/api/fred/release_series.html), [API keys](https://fred.stlouisfed.org/docs/api/api_key.html), [release index](https://fred.stlouisfed.org/releases), [state employment](https://fred.stlouisfed.org/release?rid=112), [state GDP](https://fred.stlouisfed.org/release?rid=140), [WDI](https://fred.stlouisfed.org/release?rid=234).
