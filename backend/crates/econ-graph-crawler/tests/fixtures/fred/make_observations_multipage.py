#!/usr/bin/env python3
# Copyright (c) 2024 EconGraph. All rights reserved.
# Licensed under the Microsoft Reference Source License (MS-RSL).
# See LICENSE file for complete terms and conditions.
"""Writes observations_multipage_e2e.json, the synthetic FRED series behind ECO-261's
multi-page end-to-end spec (frontend/tests/e2e/release/series-ui/multipage-series.spec.ts).

The series is long enough that the frontend reads it in three `seriesData` pages of 10,000
points (SERIES_PAGE_SIZE in frontend/src/hooks/useSeriesData.ts, the backend's cap). Its shape:

- 22,000 consecutive calendar days from 1960-01-01 (1960-01-01 .. 2020-03-25), so the pages
  hold days 0-9999, 10000-19999 and 20000-21999.
- Day i's latest value is 100 + i/100, written with two decimals ("100.00", "100.01", ...).
  Day 0 is 100.00, so "change since first available value" on day i is exactly i/100 percent.
- Missing (".") on the days in MISSING: inside a page, on both sides of each page boundary, and
  in the tail the spec's date range cuts off.
- Rows carry no `realtime_end`, which the adapter doesn't read, to keep the file small.
- An earlier vintage with a wrong value (latest + 50) on the days in REVISED, one per page, and
  an earlier "." vintage on day 12345, so only the latest revision is the right answer.

The spec computes its expected observations from this file's rows, not from these rules, so
the rules only need to hold for the spec's hard-coded sanity counts. Re-run after editing:

    python3 backend/crates/econ-graph-crawler/tests/fixtures/fred/make_observations_multipage.py
"""

import datetime
import json
import pathlib

START = datetime.date(1960, 1, 1)
DAYS = 22_000
MISSING = {777, 9_999, 10_000, 19_999, 20_000, 21_500, 21_990}
REVISED = {4_321, 15_000, 21_000}
MISSING_THEN_PUBLISHED = {12_345}
EARLIER_VINTAGE = "2025-06-02"
LATEST_VINTAGE = "2026-01-02"


def row(date: str, value: str, realtime_start: str) -> str:
    return json.dumps(
        {
            "realtime_start": realtime_start,
            "date": date,
            "value": value,
        },
        separators=(",", ":"),
    )


def main() -> None:
    rows = []
    for i in range(DAYS):
        date = (START + datetime.timedelta(days=i)).isoformat()
        latest = "." if i in MISSING else f"{100 + i / 100:.2f}"
        if i in REVISED:
            rows.append(row(date, f"{150 + i / 100:.2f}", EARLIER_VINTAGE))
        if i in MISSING_THEN_PUBLISHED:
            rows.append(row(date, ".", EARLIER_VINTAGE))
        rows.append(row(date, latest, LATEST_VINTAGE))
    header = {
        "realtime_start": "1776-07-04",
        "realtime_end": "9999-12-31",
        "observation_start": "1600-01-01",
        "observation_end": "9999-12-31",
        "units": "lin",
        "output_type": 1,
        "file_type": "json",
        "order_by": "observation_date",
        "sort_order": "asc",
        "count": len(rows),
        "offset": 0,
        "limit": 100000,
    }
    head = json.dumps(header, indent=2)[:-2]  # drop the closing "\n}"
    body = ",\n".join("    " + r for r in rows)
    out = pathlib.Path(__file__).with_name("observations_multipage_e2e.json")
    out.write_text(f'{head},\n  "observations": [\n{body}\n  ]\n}}\n')


if __name__ == "__main__":
    main()
