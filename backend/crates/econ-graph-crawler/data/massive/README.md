# Massive symbol reference

`sp500.csv` lists the securities the crawler fetches from Massive, a market data vendor: the
S&P 500 constituents. The crawler reads it at runtime (`sources/massive/symbols.rs`), from
`$MASSIVE_SYMBOLS_PATH` or, when that is unset, `massive/sp500.csv` in the crawler data
directory (`$CRAWLER_DATA_DIR`, `/app/data` in the images). It is not compiled in.

Columns: `share_class_figi,cik10,ticker,exchange,currency,name,valid_from,valid_to`.

- `share_class_figi` names the security. Series are keyed on it, so GOOG and GOOGL (one CIK)
  are two securities, and a ticker change keeps the same series.
- `cik10` is the SEC CIK, zero-padded to 10 digits, the join key to SEC data.
- `ticker` is in Massive's form: share classes take a dot (`BRK.B`, `BF.B`).
- `exchange` is the primary exchange's MIC (`XNYS`, `XNAS`, `BATS`).
- `valid_from` and `valid_to` (inclusive, empty while current) bound when the row applies. A
  ticker change closes the old row and adds a new one; rows are never overwritten, so earlier
  prices keep the ticker they traded under. A security that leaves the index is closed, not
  removed.

## Current build

| Input | Version |
| -- | -- |
| Constituents | Wikipedia, [List of S&P 500 companies](https://en.wikipedia.org/wiki/List_of_S%26P_500_companies), revision 1384572480 (2026-11-08T23:01:02Z), 503 securities |
| CIKs | SEC `company_tickers.json`, Last-Modified Fri, 06 Nov 2026 22:01:04 GMT |
| Exchanges | SEC `company_tickers_exchange.json`, Last-Modified Fri, 06 Nov 2026 22:00:41 GMT |
| Share-class FIGIs | OpenFIGI v3 mapping API (ticker, `exchCode` `US`), fetched 2026-11-09 |
| `valid_from` | 2026-11-09 |

The generator also writes the pinned copies the tests check against, in
`tests/fixtures/massive/`: `sec_company_tickers.csv` (SEC's ticker-to-CIK mapping) and
`openfigi_mapping.json` (the OpenFIGI responses).

## Regenerating

From the repository root:

```sh
python3 -I scripts/massive/generate_sp500_symbols.py --contact you@example.com --as-of YYYY-MM-DD
```

`--contact` goes in the User-Agent SEC requires. Without `OPENFIGI_API_KEY` the OpenFIGI step
takes about two minutes. The script merges with the existing file (unchanged rows keep their
`valid_from`; changed or departed ones are closed the day before `--as-of`), and prints the
input versions to copy into the table above.
