# World Bank fixtures

`gdp_per_capita.json`, `inflation.json` and `population.json` are
`/country/all/indicator/{id}` responses for the adapter's discovery and fetch tests.

The `indicator_*.json` files are recordings of `/v2/indicator/{id}?format=json`, the
per-indicator metadata that the adapter's `code_lists` and `crawler check-selection` read:

- `indicator_ny_gdp_pcap_cd.json`: a current World Development Indicator (source 2).
- `indicator_archived_ag_agr_trac_no.json`: an indicator the World Bank moved to source 57,
  "WDI Database Archives", which is no longer updated.
- `indicator_ids_dt_amt_mltc_gg_cd.json`: an International Debt Statistics indicator, which the
  API reports under source 81, "International Debt Statistics: DSSI", rather than WDI.
- `indicator_unknown.json`: the error the API returns for an id it doesn't know (message 120,
  "Invalid value"), recorded for `NOT.A.REAL.ID`.
