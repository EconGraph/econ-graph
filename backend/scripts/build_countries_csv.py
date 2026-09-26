#!/usr/bin/env python3
# Copyright (c) 2024 EconGraph. All rights reserved.
# Licensed under the Microsoft Reference Source License (MS-RSL).
# See LICENSE file for complete terms and conditions.
"""Builds crates/econ-graph-core/data/countries.csv, the shared country reference table.

Rows:
- every ISO 3166-1 country, from the iso-codes project's JSON (Debian/Ubuntu package
  `iso-codes`, /usr/share/iso-codes/json/iso_3166-1.json), keyed by ISO alpha-3;
- Kosovo (XK / XKX, user-assigned codes used by the World Bank, IMF and EU; no numeric code);
- the World Bank aggregates listed in AGGREGATES, keyed by their World Bank code, with no ISO
  codes.

Columns: key, kind, iso2, iso3, iso_numeric, wb_code, sdmx_ref_area, name, region, income_group.

Offline (the default) the script needs no network. `wb_code` defaults to the ISO alpha-3 code
(the World Bank uses alpha-3 for its economies), except for the countries in NOT_WORLD_BANK.
`sdmx_ref_area` is the ISO alpha-2 code (the CL_AREA convention of ECB, BIS and IMF).
`region` and `income_group` hold World Bank codes and are empty until a `--world-bank` run.

With `--world-bank` it fetches https://api.worldbank.org/v2/country and sets, for each country,
`wb_code` (empty when the World Bank does not list it), `region` (e.g. EAS) and `income_group`
(e.g. HIC; empty for "not classified"), and refreshes aggregate names. It prints World Bank
economies the table has no row for. This needs network access to api.worldbank.org, so it runs
at QA, not in CI. The file's header then records the fetch, and later offline rebuilds keep the
World Bank values (wb_code, region, income_group, aggregate names) instead of the defaults.

`--check` builds the table and exits 1 if its rows differ from the file's, without writing (the
comment lines, which name the iso-codes version, are not compared). After a World Bank fetch
the World Bank columns are taken from the file itself, so only the ISO-derived columns are
checked. It is a manual check, not run in CI.

Usage (from backend/):
    python3 scripts/build_countries_csv.py [--iso-json PATH] [--out PATH] [--world-bank] [--check]
"""

import argparse
import csv
import datetime
import io
import json
import re
import subprocess
import sys
import urllib.request
from pathlib import Path

DEFAULT_ISO_JSON = Path("/usr/share/iso-codes/json/iso_3166-1.json")
DEFAULT_OUT = Path(__file__).resolve().parent.parent / "crates/econ-graph-core/data/countries.csv"
WB_COUNTRY_URL = "https://api.worldbank.org/v2/country?format=json&per_page=1000"
WB_SOURCE = "World Bank API"
WB_FETCHED = re.compile(r"\(fetched \d{4}-\d{2}-\d{2}\)")

COLUMNS = [
    "key", "kind", "iso2", "iso3", "iso_numeric", "wb_code",
    "sdmx_ref_area", "name", "region", "income_group",
]

# World Bank aggregates carried as rows (key = World Bank code).
AGGREGATES = [
    ("WLD", "World"),
    ("EMU", "Euro area"),
    ("EUU", "European Union"),
    ("HIC", "High income"),
    ("UMC", "Upper middle income"),
    ("LMC", "Lower middle income"),
    ("LIC", "Low income"),
    ("MIC", "Middle income"),
    ("LMY", "Low & middle income"),
]

# Countries not in ISO 3166-1: (iso2, iso3, name).
EXTRA_COUNTRIES = [("XK", "XKX", "Kosovo")]

# Countries the World Bank does not publish, so they get no default wb_code.
NOT_WORLD_BANK = {"TWN"}

# World Bank income level for "not classified"; stored as no income group.
WB_NOT_CLASSIFIED = "INX"


def country_rows(iso_json: Path) -> list[dict]:
    try:
        entries = json.loads(iso_json.read_text(encoding="utf-8"))["3166-1"]
    except FileNotFoundError:
        sys.exit(f"{iso_json} not found: install the iso-codes package or pass --iso-json")
    rows = [
        country_row(e["alpha_2"], e["alpha_3"], e["numeric"], e.get("common_name") or e["name"])
        for e in entries
    ]
    rows += [country_row(iso2, iso3, "", name) for iso2, iso3, name in EXTRA_COUNTRIES]
    return sorted(rows, key=lambda r: r["key"])


def country_row(iso2: str, iso3: str, numeric: str, name: str) -> dict:
    return {
        "key": iso3, "kind": "country", "iso2": iso2, "iso3": iso3,
        "iso_numeric": numeric, "wb_code": "" if iso3 in NOT_WORLD_BANK else iso3,
        "sdmx_ref_area": iso2, "name": name, "region": "", "income_group": "",
    }


def aggregate_rows() -> list[dict]:
    return [
        {**{c: "" for c in COLUMNS}, "key": code, "kind": "aggregate",
         "wb_code": code, "name": name}
        for code, name in AGGREGATES
    ]


def split_file(text: str) -> tuple[list[str], list[str]]:
    """Returns the file's comment lines and its CSV lines."""
    lines = text.splitlines()
    return ([l for l in lines if l.startswith("#")], [l for l in lines if not l.startswith("#")])


def read_existing(path: Path) -> tuple[dict[str, dict], str | None]:
    """The current file's rows by key, and the World Bank fetch its header records, if any."""
    if not path.exists():
        return {}, None
    comments, data = split_file(path.read_text(encoding="utf-8"))
    wb = next((m.group(0) for c in comments if (m := WB_FETCHED.search(c))), None)
    return {r["key"]: r for r in csv.DictReader(data)}, wb


def keep_world_bank(rows: list[dict], existing: dict[str, dict]) -> None:
    """Keeps the values an earlier --world-bank run wrote, instead of the offline defaults."""
    for r in rows:
        old = existing.get(r["key"])
        if not old:
            print(f"{r['key']} is new since the World Bank fetch: rerun with --world-bank",
                  file=sys.stderr)
        else:
            for col in ("wb_code", "region", "income_group"):
                r[col] = old.get(col) or ""
            if r["kind"] == "aggregate":
                r["name"] = old.get("name") or r["name"]


def apply_world_bank(rows: list[dict]) -> None:
    economies = []
    page, pages = 1, 1
    while page <= pages:
        with urllib.request.urlopen(f"{WB_COUNTRY_URL}&page={page}", timeout=60) as resp:
            body = json.load(resp)
        if not (isinstance(body, list) and len(body) == 2 and isinstance(body[1], list)):
            sys.exit(f"unexpected World Bank response on page {page}: {json.dumps(body)[:500]}")
        meta, items = body
        pages = int(meta["pages"])
        economies.extend(items)
        page += 1
    by_code = {e["id"]: e for e in economies}
    for r in rows:
        if r["kind"] == "country":
            wb = by_code.get(r["iso3"])
            income = wb["incomeLevel"]["id"] if wb else ""
            r["wb_code"] = wb["id"] if wb else ""
            r["region"] = wb["region"]["id"] if wb else ""
            r["income_group"] = "" if income == WB_NOT_CLASSIFIED else income
        elif (wb := by_code.get(r["key"])) is not None:
            r["name"] = wb["name"]
        else:
            print(f"World Bank aggregate {r['key']} not in the response", file=sys.stderr)
    known = {r["wb_code"] for r in rows}
    for e in economies:
        if e["region"]["id"] != "NA" and e["id"] not in known:
            print(f"World Bank economy without a row: {e['id']} {e['name']}", file=sys.stderr)


def installed_iso_codes_version() -> str:
    try:
        return subprocess.run(["dpkg-query", "-W", "-f=${Version}", "iso-codes"],
                              capture_output=True, text=True, check=True).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return "(version unknown)"


def render(rows: list[dict], iso_version: str, wb_fetched: str | None) -> str:
    source = f"iso-codes {iso_version}"
    if wb_fetched:
        source += f" and the {WB_SOURCE} {wb_fetched}"
    out = io.StringIO()
    out.write("# Country and aggregate reference table, read at runtime from $REFERENCE_DATA_DIR.\n")
    out.write(f"# Generated by backend/scripts/build_countries_csv.py from {source}; "
              "do not edit by hand.\n")
    w = csv.DictWriter(out, fieldnames=COLUMNS, lineterminator="\n")
    w.writeheader()
    w.writerows(rows)
    return out.getvalue()


def main() -> int:
    p = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    p.add_argument("--iso-json", type=Path, default=DEFAULT_ISO_JSON)
    p.add_argument("--iso-version",
                   help="iso-codes version recorded in the file (default: the installed package's)")
    p.add_argument("--out", type=Path, default=DEFAULT_OUT)
    p.add_argument("--world-bank", action="store_true",
                   help="fill wb_code, region and income_group from api.worldbank.org")
    p.add_argument("--check", action="store_true",
                   help="exit 1 if the file's rows differ from a fresh build; write nothing")
    args = p.parse_args()

    rows = country_rows(args.iso_json) + aggregate_rows()
    existing, wb_fetched = read_existing(args.out)
    if args.world_bank:
        apply_world_bank(rows)
        wb_fetched = f"(fetched {datetime.date.today().isoformat()})"
    elif wb_fetched:
        keep_world_bank(rows, existing)

    text = render(rows, args.iso_version or installed_iso_codes_version(), wb_fetched)
    if args.check:
        current = args.out.read_text(encoding="utf-8") if args.out.exists() else ""
        if split_file(current)[1] != split_file(text)[1]:
            print(f"{args.out} is out of date; rerun {Path(__file__).name}", file=sys.stderr)
            return 1
        return 0
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(text, encoding="utf-8")
    print(f"wrote {len(rows)} rows to {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
