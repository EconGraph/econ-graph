# Bank of Canada Valet fixtures

Every file here is a **recording**: the unedited body Valet returned on 2026-10-30 for the
request below. The tests in [`src/sources/boc/tests.rs`](../../../src/sources/boc/tests.rs)
also build a few synthetic bodies inline (missing values, malformed dates and values); those
are not recordings.

| File | Request |
|---|---|
| `series_V39079.json` | `GET https://www.bankofcanada.ca/valet/series/V39079/json` |
| `series_FXUSDCAD.json` | `GET https://www.bankofcanada.ca/valet/series/FXUSDCAD/json` |
| `series_V41552801.json` | `GET https://www.bankofcanada.ca/valet/series/V41552801/json` |
| `observations_FXUSDCAD.json` | `GET https://www.bankofcanada.ca/valet/observations/FXUSDCAD/json?start_date=2026-10-19&end_date=2026-10-29` |
| `observations_V41552801.json` | `GET https://www.bankofcanada.ca/valet/observations/V41552801/json?start_date=2026-03-01&end_date=2026-08-01` |
| `error_series_not_found.json` | `GET https://www.bankofcanada.ca/valet/observations/NOPE123/json` (HTTP 404) |

`V41552801` (M2++ (gross), seasonally adjusted) is one of the series whose `metadata` Valet
leaves empty, so its fixtures cover the adapter's fallback to `dateFormat` for the frequency
and no units. The adapter never sends `end_date`; it is used here only to keep the recordings
short.
