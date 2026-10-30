#!/usr/bin/env python3
# Copyright (c) 2024 EconGraph. All rights reserved.
# Licensed under the Microsoft Reference Source License (MS-RSL).
# See LICENSE file for complete terms and conditions.
"""Writes observations_usrec_e2e.json, the FRED recession indicator (USREC) the release
end-to-end stack is seeded with.

The file has FRED's documented `/fred/series/observations` shape
(https://fred.stlouisfed.org/docs/api/fred/series_observations.html). It is built here rather
than recorded from the live API:

- One row per month from 1854-12-01 through LAST_MONTH, the window FRED's USREC covers.
- Value "1" in the months listed in RECESSIONS and "0" elsewhere. USREC uses the trough
  method: a recession runs from the month after an NBER business-cycle peak through the
  trough month, both inclusive. The first range is the December 1854 trough alone, since the
  series starts there. The ranges match FRED's published USREC values.
- A single vintage, VINTAGE, for every row (vintagedates_usrec_e2e.json lists it). The real
  series has more vintages; the specs only read the latest values.
- Rows carry no `realtime_end`, which the adapter doesn't read, to keep the file small.

Re-run after editing:

    python3 backend/crates/econ-graph-crawler/tests/fixtures/fred/make_observations_usrec.py
"""

import json
import pathlib

FIRST_MONTH = (1854, 12)
LAST_MONTH = (2026, 9)
VINTAGE = "2026-10-01"

# (first month valued 1, last month valued 1), as "YYYY-MM".
RECESSIONS = [
    ("1854-12", "1854-12"),
    ("1857-07", "1858-12"),
    ("1860-11", "1861-06"),
    ("1865-05", "1867-12"),
    ("1869-07", "1870-12"),
    ("1873-11", "1879-03"),
    ("1882-04", "1885-05"),
    ("1887-04", "1888-04"),
    ("1890-08", "1891-05"),
    ("1893-02", "1894-06"),
    ("1896-01", "1897-06"),
    ("1899-07", "1900-12"),
    ("1902-10", "1904-08"),
    ("1907-06", "1908-06"),
    ("1910-02", "1912-01"),
    ("1913-02", "1914-12"),
    ("1918-09", "1919-03"),
    ("1920-02", "1921-07"),
    ("1923-06", "1924-07"),
    ("1926-11", "1927-11"),
    ("1929-09", "1933-03"),
    ("1937-06", "1938-06"),
    ("1945-03", "1945-10"),
    ("1948-12", "1949-10"),
    ("1953-08", "1954-05"),
    ("1957-09", "1958-04"),
    ("1960-05", "1961-02"),
    ("1970-01", "1970-11"),
    ("1973-12", "1975-03"),
    ("1980-02", "1980-07"),
    ("1981-08", "1982-11"),
    ("1990-08", "1991-03"),
    ("2001-04", "2001-11"),
    ("2008-01", "2009-06"),
    ("2020-03", "2020-04"),
]


def months():
    year, month = FIRST_MONTH
    while (year, month) <= LAST_MONTH:
        yield f"{year:04d}-{month:02d}"
        month += 1
        if month == 13:
            year, month = year + 1, 1


def in_recession(month: str) -> bool:
    return any(first <= month <= last for first, last in RECESSIONS)


def main() -> None:
    rows = [
        json.dumps(
            {
                "realtime_start": VINTAGE,
                "date": f"{month}-01",
                "value": "1" if in_recession(month) else "0",
            },
            separators=(",", ":"),
        )
        for month in months()
    ]
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
    head = json.dumps(header, indent=2)[:-2]
    text = head + ',\n  "observations": [\n    ' + ",\n    ".join(rows) + "\n  ]\n}\n"
    json.loads(text)
    out = pathlib.Path(__file__).with_name("observations_usrec_e2e.json")
    out.write_text(text)
    print(f"wrote {out} ({len(rows)} rows)")


if __name__ == "__main__":
    main()
