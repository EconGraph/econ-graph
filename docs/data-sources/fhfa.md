# FHFA House Price Index

Adapter: `backend/crates/econ-graph-crawler/src/sources/fhfa.rs`. Fixtures:
`tests/fixtures/fhfa/`. No key.

> **Release 1 (#240, draft):** the API client below is replaced by a download of the HPI
> master CSV, as dataset `fhfa_hpi` with 203 series. Train 1 covers the purchase-only
> and all-transactions indexes for the US, census divisions and states, NSA and SA;
> metros later. The dimensions come from the file's columns, as proposed at the end of
> this page, and the made-up `{CODE}HPI` ids are dropped.

## What we fetch today

- **Discovery**: no request. A static catalog: the national index `USHPI`, one `{STATE}HPI`
  per state and DC (from `data/us_states.csv`), and one `{METRO}HPI` for each of a short
  list of metro areas. All quarterly, units "Index (1991Q1 = 100)". Metro codes that
  collide with state codes lose (`LAHPI` is Louisiana, not Los Angeles).
- **Fetch**: `GET https://api.fhfa.gov/v1/house-price-index/{national | state/{CODE} |
  metro/{CODE}}?page=N[&start_date=YYYY-MM-DD]`, paged.
- **Kept**: `year` + `quarter` (as the quarter's first day) and `hpi_value`.
- **Dropped**: `yoy_change`, `qoq_change` (derivable), `state`, `metro_area`.

**This endpoint has never been verified.** The request and response shape were carried over
from structs in the old code, which never ran against a live server. FHFA publishes the HPI
as downloadable files, not (as far as its public docs show) as a JSON API. The fixtures
below are hand-written to that assumed shape, not recordings.

## Sample

Assumed API shape (`hpi_ca_page1.json`, **not a real recording**):

```json
{
  "data": [
    {"year": 2024, "quarter": 1, "state": "CA", "metro_area": null, "hpi_value": 1012.4, "yoy_change": 5.2, "qoq_change": 1.1}
  ],
  "meta": {"total_count": 3, "page": 1, "per_page": 2}
}
```

What FHFA actually publishes (**from public docs**): a master CSV of all HPI series,
`HPI_master.csv`, with one row per series and period (values illustrative):

```csv
hpi_type,hpi_flavor,frequency,level,place_name,place_id,yr,period,index_nsa,index_sa
traditional,purchase-only,monthly,USA or Census Division,United States,USA,2024,3,420.1,418.7
traditional,all-transactions,quarterly,State,California,CA,2024,1,1012.4,
```

`index_sa` is only present for some series (purchase-only, national and division level).

## Frequency and revisions

Monthly (purchase-only, national and census divisions) and quarterly (all levels). The
index is a repeat-sales estimate, so every release re-estimates the whole history as new
sale pairs arrive (from public docs): each quarterly release is effectively a new vintage
of every observation. The file has no vintage column; the vintage is the release date.

## Flags and footnotes

None per observation in the published file. Series with too few transactions are omitted
rather than flagged.

## Proposed dataset mapping

One dataset for the HPI master file:

| Role | Columns |
|---|---|
| Dimensions | `hpi_type`, `hpi_flavor`, `frequency`, `level`, `place_id` |
| Measures | `index_nsa`, `index_sa` (decimal; `index_sa` often null) |
| Attributes | none |
| Shape | wide (two measures), `revision_date` = release date |

The practical next step is to replace the API adapter with a download of the master file:
one request gives every series, and the dimension columns replace our made-up `{CODE}HPI`
ids.
