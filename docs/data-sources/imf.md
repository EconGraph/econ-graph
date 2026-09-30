# IMF (International Monetary Fund)

No adapter (removed in #217): its discovery used made-up series ids and could never fetch
data. `SourceId::Imf` stays reserved for a later SDMX adapter. The rest of this page is
historical reference for that future adapter, not a description of anything on `main` today.

## What the old adapter did

- **Discovery**: `GET http://dataservices.imf.org/REST/SDMX_JSON.svc/Dataflow`, filtered by
  keyword, then each matching dataset (IFS, BOP, GFS, WEO) expanded from a hard-coded list
  (`known_series`) of United States series. The ids (`IFS_US_PCPI_IX`, `WEO_US_NGDP_RPCH`)
  are our own; they resemble SDMX keys but are not one.
- **Fetch**: not implemented; nothing has requested or parsed `CompactData`.

The base URL is IMF's legacy SDMX 2.0 JSON service. IMF has moved its data to a new portal
and SDMX 3.0 API and has been retiring the legacy service (from public docs); check which
one is live before building on it.

## Sample

Catalog (**recorded**, `dataflow.json`, trimmed):

```json
{"Structure": {"Dataflows": {"Dataflow": [
  {"@id": "DS-IFS", "Name": [{"@xml:lang": "en", "$": "International Financial Statistics (IFS)"}],
   "KeyFamilyRef": {"KeyFamilyID": "IFS", "KeyFamilyAgencyID": "IMF"}},
  {"@id": "DS-WEO", "Name": {"@xml:lang": "en", "#text": "World Economic Outlook (WEO)"},
   "KeyFamilyRef": {"KeyFamilyID": "WEO", "KeyFamilyAgencyID": "IMF"}}
]}}}
```

Data (**from public docs**, legacy service):
`GET CompactData/IFS/M.US.PCPI_IX` returns (values illustrative)

```json
{"CompactData": {"DataSet": {"Series": {
  "@FREQ": "M", "@REF_AREA": "US", "@INDICATOR": "PCPI_IX",
  "@UNIT_MULT": "0", "@TIME_FORMAT": "P1M",
  "Obs": [
    {"@TIME_PERIOD": "2024-01", "@OBS_VALUE": "308.417"},
    {"@TIME_PERIOD": "2024-02", "@OBS_VALUE": "310.326"}
  ]
}}}}
```

The key `M.US.PCPI_IX` is the dataset's dimensions in order (frequency, area, indicator).
The dataset's data structure definition (`DataStructure/IFS`) lists its dimensions,
attributes and code lists.

## Frequency and revisions

IFS and BOP: monthly, quarterly and annual, revised as countries report. WEO: annual
values, but the dataset itself is published as vintages (April and October editions, each
with forecasts), so a WEO vintage is the edition, not the crawl.

## Flags and footnotes

SDMX attributes at series level (`UNIT_MULT`, `TIME_FORMAT`, base year) and observation
level (`OBS_STATUS`, and in some datasets a comment or footnote attribute; from public
docs). None are parsed yet.

## Proposed dataset mapping

IMF data is already SDMX, so the mapping is the dataset's own structure. For IFS:

| Role | Columns |
|---|---|
| Dimensions | `freq`, `ref_area`, `indicator` (from the DSD) |
| Measures | `value` (decimal, from `OBS_VALUE`) |
| Attributes | `obs_status`; series-level `unit_mult` stays series metadata |
| Shape | long (IFS has thousands of indicators), `revision_date` = crawl date; for WEO, the edition date |

Discovery should read each dataset's DSD and code lists instead of the hard-coded list, and
ids should become SDMX keys. The same approach covers the other SDMX publishers we list as
static catalogs (ECB, OECD, ILO, UN).
