# WTO Timeseries fixtures

These files are hand-shaped, not recordings of the live API. Their values are synthetic and
their indicator names and descriptions are illustrative: the adapter takes real names, units
and frequencies from the API's `/indicators` response at crawl time.

Their shape follows two public sources:

- The response schemas the WTO API portal publishes for the Timeseries API v1 (the
  `get-indicators` and `get-data-i-i` operations, readable at
  `https://apiportal.wto.org/developer/apis/version1/operations/{operation}?api-version=2022-04-01-preview`).
  `indicators_*.json` carries every field of the portal's `/indicators` schema. The `data_*.json`
  rows carry every field of its `DataPointExtended` schema, written with the machine-readable
  (`head=M`) column names.
- The `/data` envelope, `{"Dataset": [...]}`, and the `head=M` column names
  (`ReportingEconomyCode`, `PeriodCode`, `Year`, `Value`, ...), which the portal's schema does
  not show. Several independent, public WTO API clients read the API this way.

| File | Response it stands for |
| --- | --- |
| `indicators_its_mtv_ax.json` | `/indicators?i=ITS_MTV_AX` |
| `data_its_mtv_ax.json` | `/data?i=ITS_MTV_AX&pc=TO&p=000&...` |
| `indicators_tp_a_0010.json` | `/indicators?i=TP_A_0010` |
| `data_tp_a_0010.json` | `/data?i=TP_A_0010&...` (no product or partner dimension) |

`data_its_mtv_ax.json` also exercises the parser's edge cases, each in one row: a `null` value,
a row for another product (`AG`), a numeric `ReportingEconomyCode` (`4`) with a string `Year`
and `Value`, the world (`000`) and European Union (`918`) aggregates, and an economic group
(`975`) that the shared country table does not list.
