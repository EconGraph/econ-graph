# Massive fixtures

Responses of Massive, a market data vendor, for the adapter's unit tests
(`src/sources/massive/adapter/tests.rs`). They were written by hand, not recorded from the
live API, because no key was available when they were made. Their shape follows Massive's
public REST documentation (`https://massive.com/docs/rest/stocks/aggregates/`) and the API
facts in ECO-212. The prices are illustrative, and so are the request ids.

| File | Response it stands for |
| --- | --- |
| `grouped_2024-01-03.json` | `/v2/aggs/grouped/locale/us/market/stocks/2024-01-03?adjusted=false`: a normal day |
| `grouped_2024-01-01_holiday.json` | the same for New Year's Day: no `results` |
| `ticker_aapl_2024-01-02_2024-01-03.json` | `/v2/aggs/ticker/AAPL/range/1/day/2024-01-02/2024-01-03?adjusted=false&sort=asc&limit=50000` |
| `ticker_no_data.json` | a per-ticker range with no bars |
| `error_401.json` | `401`: an unknown key |
| `error_403_not_entitled.json` | `403`: the plan isn't entitled to the data |
| `error_403_timeframe.json` | `403`: the date is older than the plan's history |
| `error_429.json` | `429`: too many requests |
| `malformed.txt` | a body cut off mid-object |
| `symbols.csv` | a symbol table in the shipped file's format |

`grouped_2024-01-03.json` also covers the parser's edge cases:

- a volume written with an exponent (`5.8414460e+07`) and a fractional one (`25933.6`);
- a bar with no `vw` (MSFT);
- two share classes of one company (GOOGL and GOOG);
- a ticker that isn't in the symbol table (ZZZZ).

Grouped bars carry `t` at the end of the trading day, 16:00 in New York. Per-ticker bars
carry it at the start, midnight in New York.
