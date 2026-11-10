#!/usr/bin/env python3
"""Regenerate the S&P 500 symbol reference for Massive, a market data vendor.

Writes, relative to the repository root:

- backend/crates/econ-graph-crawler/data/massive/sp500.csv, the symbol file the crawler loads
  at runtime (sources/massive/symbols.rs).
- backend/crates/econ-graph-crawler/tests/fixtures/massive/sec_company_tickers.csv, a pinned copy
  of SEC's ticker-to-CIK mapping (cik10,ticker for every row of company_tickers.json).
- backend/crates/econ-graph-crawler/tests/fixtures/massive/openfigi_mapping.json, the OpenFIGI
  mapping responses the share-class FIGIs came from.

Sources:

- S&P 500 constituents: Wikipedia's "List of S&P 500 companies" (the `constituents` table),
  via the MediaWiki parse API so the revision id and timestamp are recorded.
- CIKs: SEC's company_tickers.json. Exchanges: SEC's company_tickers_exchange.json, written as
  the exchange's MIC (XNYS, XNAS, BATS), which is how Massive names primary exchanges.
- Share-class FIGIs: the OpenFIGI v3 mapping API, ticker plus exchCode US. It works without a
  key at 25 requests a minute and 10 jobs a request; set OPENFIGI_API_KEY for higher limits.

Tickers are written the way Massive expects them: share classes with a dot (BRK.B, BF.B).
SEC writes them with a dash (BRK-B) and OpenFIGI with a slash (BRK/B); the script converts.

Existing rows are not overwritten. A ticker already in the file with the same FIGI keeps its row
(and valid_from). A constituent whose FIGI is new gets a row with valid_from set to --as-of. A
ticker change (same FIGI, new ticker) closes the old row (valid_to = --as-of minus one day) and
adds a new one. Rows for securities that left the index are kept and closed the same way.

Usage (from the repository root):

    python3 -I scripts/massive/generate_sp500_symbols.py --contact you@example.com [--as-of YYYY-MM-DD]

SEC requires a User-Agent naming the requester and a contact address; --contact sets it.
"""

import argparse
import csv
import datetime as dt
import html.parser
import io
import json
import os
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
CRATE = REPO / "backend" / "crates" / "econ-graph-crawler"
SYMBOLS_CSV = CRATE / "data" / "massive" / "sp500.csv"
FIXTURES = CRATE / "tests" / "fixtures" / "massive"
SEC_FIXTURE = FIXTURES / "sec_company_tickers.csv"
FIGI_FIXTURE = FIXTURES / "openfigi_mapping.json"

HEADER = ["share_class_figi", "cik10", "ticker", "exchange", "currency", "name", "valid_from", "valid_to"]

WIKI_API = "https://en.wikipedia.org/w/api.php"
WIKI_PAGE = "List of S&P 500 companies"
SEC_TICKERS = "https://www.sec.gov/files/company_tickers.json"
SEC_EXCHANGE = "https://www.sec.gov/files/company_tickers_exchange.json"
OPENFIGI = "https://api.openfigi.com/v3/mapping"

# SEC exchange names to the MIC Massive reports as a ticker's primary exchange.
EXCHANGE_MIC = {"NYSE": "XNYS", "Nasdaq": "XNAS", "CBOE": "BATS"}


def fetch(url, user_agent, data=None, headers=None):
    req = urllib.request.Request(url, data=data, headers={"User-Agent": user_agent, **(headers or {})})
    with urllib.request.urlopen(req, timeout=60) as resp:
        return resp.read(), resp.headers


class ConstituentsTable(html.parser.HTMLParser):
    """Collects the cell text of each row of the table with id="constituents"."""

    def __init__(self):
        super().__init__()
        self.depth = 0  # table nesting depth while inside the constituents table
        self.rows = []
        self.cell = None

    def handle_starttag(self, tag, attrs):
        if tag == "table":
            if self.depth or dict(attrs).get("id") == "constituents":
                self.depth += 1
        elif self.depth == 1 and tag == "tr":
            self.rows.append([])
        elif self.depth == 1 and tag in ("td", "th"):
            self.cell = []

    def handle_endtag(self, tag):
        if tag == "table" and self.depth:
            self.depth -= 1
        elif self.depth == 1 and tag in ("td", "th") and self.cell is not None:
            self.rows[-1].append(" ".join("".join(self.cell).split()))
            self.cell = None

    def handle_data(self, data):
        if self.cell is not None:
            self.cell.append(data)


def constituents(user_agent):
    """Returns (rows, revid, timestamp); each row is (ticker, name, cik10) from Wikipedia."""
    q = urllib.parse.urlencode(
        {"action": "parse", "page": WIKI_PAGE, "prop": "text|revid", "format": "json", "formatversion": "2"}
    )
    body, _ = fetch(f"{WIKI_API}?{q}", user_agent)
    parsed = json.loads(body)["parse"]
    revid = parsed["revid"]
    q = urllib.parse.urlencode(
        {"action": "query", "revids": revid, "prop": "revisions", "rvprop": "timestamp", "format": "json",
         "formatversion": "2"}
    )
    body, _ = fetch(f"{WIKI_API}?{q}", user_agent)
    timestamp = json.loads(body)["query"]["pages"][0]["revisions"][0]["timestamp"]

    table = ConstituentsTable()
    table.feed(parsed["text"])
    header, *rows = [r for r in table.rows if r]
    i_sym, i_name, i_cik = header.index("Symbol"), header.index("Security"), header.index("CIK")
    out = [(r[i_sym], r[i_name], r[i_cik].zfill(10)) for r in rows]
    if not 490 <= len(out) <= 520:
        sys.exit(f"expected about 503 constituents, parsed {len(out)}")
    return out, revid, timestamp


def openfigi(tickers, api_key, user_agent):
    """Maps each Massive-style ticker to its share-class FIGI. Returns (figis, raw responses)."""
    batch = 100 if api_key else 10
    pause = 0.25 if api_key else 2.5  # 25 requests per 6 s with a key, per minute without
    headers = {"Content-Type": "application/json"}
    if api_key:
        headers["X-OPENFIGI-APIKEY"] = api_key
    figis, raw = {}, []
    for start in range(0, len(tickers), batch):
        chunk = tickers[start:start + batch]
        jobs = [{"idType": "TICKER", "idValue": t.replace(".", "/"), "exchCode": "US"} for t in chunk]
        results = None
        for attempt in range(5):
            try:
                body, _ = fetch(OPENFIGI, user_agent, json.dumps(jobs).encode(), headers)
                results = json.loads(body)
                break
            except urllib.error.HTTPError as e:
                if e.code != 429 or attempt == 4:
                    raise
                time.sleep(60)
        if results is None:
            sys.exit("OpenFIGI: no response after retries")
        for ticker, job, result in zip(chunk, jobs, results):
            raw.append({"job": job, "result": result})
            share = {d["shareClassFIGI"] for d in result.get("data", []) if d.get("shareClassFIGI")}
            if len(share) != 1:
                sys.exit(f"{ticker}: expected one share-class FIGI from OpenFIGI, got {sorted(share) or result}")
            figis[ticker] = share.pop()
        time.sleep(pause)
    return figis, raw


def read_existing():
    if not SYMBOLS_CSV.exists():
        return []
    with SYMBOLS_CSV.open(newline="") as f:
        return list(csv.DictReader(f))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--contact", required=True, help="contact email for the SEC User-Agent")
    ap.add_argument("--as-of", default=dt.date.today().isoformat(), help="valid_from for new rows (default today)")
    args = ap.parse_args()
    as_of = dt.date.fromisoformat(args.as_of)
    user_agent = f"EconGraph S&P 500 symbol generator ({args.contact})"

    members, revid, rev_ts = constituents(user_agent)

    sec_body, sec_headers = fetch(SEC_TICKERS, user_agent)
    sec = json.loads(sec_body).values()
    sec_cik = {r["ticker"]: str(r["cik_str"]).zfill(10) for r in sec}
    exch_body, exch_headers = fetch(SEC_EXCHANGE, user_agent)
    exch = json.loads(exch_body)
    fields = exch["fields"]
    sec_exchange = {r[fields.index("ticker")]: r[fields.index("exchange")] for r in exch["data"]}

    rows = []
    for ticker, name, wiki_cik in members:
        sec_ticker = ticker.replace(".", "-")
        cik = sec_cik.get(sec_ticker)
        if cik is None:
            sys.exit(f"{ticker}: not in SEC company_tickers.json")
        if cik != wiki_cik:
            sys.exit(f"{ticker}: SEC CIK {cik} differs from Wikipedia's {wiki_cik}")
        sec_exch = sec_exchange.get(sec_ticker, "")
        exchange = EXCHANGE_MIC.get(sec_exch)
        if exchange is None:
            sys.exit(f"{ticker}: no known exchange in SEC data ({sec_exch!r})")
        rows.append({"cik10": cik, "ticker": ticker, "exchange": exchange, "currency": "USD", "name": name})

    figis, figi_raw = openfigi([r["ticker"] for r in rows], os.environ.get("OPENFIGI_API_KEY"), user_agent)
    for r in rows:
        r["share_class_figi"] = figis[r["ticker"]]

    # Merge with the existing file: keep open rows that still match, close the rest.
    current = {r["share_class_figi"]: r for r in rows}
    closed_on = (as_of - dt.timedelta(days=1)).isoformat()
    out, kept = [], set()
    for old in read_existing():
        if old["valid_to"]:
            out.append(old)
            continue
        new = current.get(old["share_class_figi"])
        if new and new["ticker"] == old["ticker"]:
            out.append(old)
            kept.add(old["share_class_figi"])
        else:
            out.append({**old, "valid_to": closed_on})
    for figi, r in current.items():
        if figi not in kept:
            out.append({**r, "valid_from": as_of.isoformat(), "valid_to": ""})
    out.sort(key=lambda r: (r["ticker"], r["valid_from"]))

    SYMBOLS_CSV.parent.mkdir(parents=True, exist_ok=True)
    with SYMBOLS_CSV.open("w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=HEADER, lineterminator="\n")
        w.writeheader()
        w.writerows(out)

    FIXTURES.mkdir(parents=True, exist_ok=True)
    buf = io.StringIO()
    w = csv.writer(buf, lineterminator="\n")
    w.writerow(["cik10", "ticker"])
    w.writerows(sorted({(str(r["cik_str"]).zfill(10), r["ticker"]) for r in sec}))
    SEC_FIXTURE.write_text(buf.getvalue())
    FIGI_FIXTURE.write_text(json.dumps(figi_raw, indent=1, sort_keys=True) + "\n")

    print(f"constituents: Wikipedia '{WIKI_PAGE}' revision {revid} ({rev_ts})")
    print(f"SEC company_tickers.json Last-Modified: {sec_headers.get('Last-Modified')}")
    print(f"SEC company_tickers_exchange.json Last-Modified: {exch_headers.get('Last-Modified')}")
    print(f"wrote {len(out)} rows to {SYMBOLS_CSV.relative_to(REPO)}")
    print("update data/massive/README.md with the dates above")


if __name__ == "__main__":
    main()
