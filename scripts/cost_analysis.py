#!/usr/bin/env python3
"""Deterministic USD cost model; no external packages or terminal-output parsing."""
import argparse
import csv
from datetime import datetime, timezone
from decimal import Decimal, ROUND_HALF_UP
import json
from pathlib import Path
import re
import subprocess
import sys

from codebase_stats import CATEGORIES, collect

D = Decimal
CENT = D('0.01')
TOKEN_COLUMNS = ['Input (w/ Cache Write)', 'Input (w/o Cache Write)', 'Cache Read', 'Output Tokens']
DOCUMENTS = ['docs/archive/generated/development-costs/analysis.md']


def validate_stats(stats):
    expected = set(CATEGORIES) | {'total_lines'}
    if not isinstance(stats, dict) or set(stats) != expected:
        raise ValueError('Missing or unexpected statistics fields')
    if any(type(value) is not int or value < 0 for value in stats.values()):
        raise ValueError('Statistics must be nonnegative JSON integers')
    if stats['total_lines'] != sum(stats[k] for k in CATEGORIES):
        raise ValueError('Total lines do not reconcile to category counts')
    if stats['total_lines'] == 0:
        raise ValueError('Empty codebase: percentages and cost per line are undefined')
    return stats


def read_usage(path):
    tokens, cost, requests = 0, D(0), 0
    with path.open(newline='', encoding='utf-8-sig') as source:
        reader = csv.DictReader(source, strict=True)
        required = {'Date', 'Kind', 'Model', 'Max Mode', 'Total Tokens', 'Cost', *TOKEN_COLUMNS}
        if reader.fieldnames is None or len(set(reader.fieldnames)) != len(reader.fieldnames) or set(reader.fieldnames) != required:
            raise ValueError('Missing, duplicate, or unexpected usage CSV columns')
        for line, row in enumerate(reader, 2):
            if None in row or any(value is None or not value.strip() for value in row.values()):
                raise ValueError(f'Missing or extra CSV values at row {line}')
            counts = []
            for column in [*TOKEN_COLUMNS, 'Total Tokens']:
                if not re.fullmatch(r'[0-9]+', row[column]):
                    raise ValueError(f'Invalid {column} at CSV row {line}')
                counts.append(int(row[column]))
            if sum(counts[:-1]) != counts[-1]:
                raise ValueError(f'Token counts do not reconcile at CSV row {line}')
            if not re.fullmatch(r'[0-9]+(?:\.[0-9]+)?', row['Cost']):
                raise ValueError(f'Invalid cost at CSV row {line}')
            amount = D(row['Cost'])
            if not amount.is_finite():
                raise ValueError(f'Nonfinite cost at CSV row {line}')
            # Preserve the model's sum of reported costs, including Not Charged rows.
            # This is a usage estimate, not a verified invoice or incremental bill.
            cost += amount
            tokens += counts[-1]
            requests += 1
    if requests == 0:
        raise ValueError('Usage CSV contains no records')
    return {'total_cost': cost, 'total_tokens': tokens, 'total_requests': requests}


def money(value):
    return value.quantize(CENT, rounding=ROUND_HALF_UP)


def calculate(stats, usage):
    validate_stats(stats)
    if (type(usage['total_tokens']) is not int or usage['total_tokens'] < 0
            or type(usage['total_requests']) is not int or usage['total_requests'] <= 0
            or not isinstance(usage['total_cost'], D) or not usage['total_cost'].is_finite()
            or usage['total_cost'] < 0):
        raise ValueError('Invalid usage totals')
    production = stats['backend_production'] + stats['frontend_production']
    tests = stats['backend_tests'] + stats['frontend_tests']
    infrastructure = stats['configuration'] + stats['scripts']
    breakdown = {'production': money(production * D('2.50')), 'tests': money(tests * D('1.25')),
                 'infrastructure': money(infrastructure * D('1.00')),
                 'documentation': money(stats['documentation'] * D('0.75'))}
    base = sum(breakdown.values())
    # Round once at each displayed monetary component so totals reconcile exactly.
    overhead = money(base * D('0.75'))
    traditional = base + overhead
    tools = money(usage['total_cost']) + D('20.00')
    ai = D('33600.00') + tools
    savings = traditional - ai
    if traditional <= 0:
        raise ValueError('Traditional cost denominator must be positive')
    result = {
        'codebase_stats': {**{key: stats[key] for key in sorted(stats)}, 'infrastructure': infrastructure},
        'cost_analysis': {
            'traditional_development': {'line_costs': breakdown, 'base_cost': base, 'overhead_cost': overhead, 'total_cost': traditional},
            'ai_assisted_development': {'staff_engineer_cost': D('33600.00'), 'ai_tool_cost': tools, 'total_cost': ai},
            'savings': {'dollar_amount': savings, 'percentage': (savings * 100 / traditional).quantize(D('0.1'), rounding=ROUND_HALF_UP)}},
        'ai_usage': usage,
    }
    # Exact decimal arithmetic above; JSON uses finite numbers, currencies at cents.
    def numbers(value):
        if isinstance(value, dict):
            return {key: numbers(item) for key, item in value.items()}
        if isinstance(value, D):
            if not value.is_finite():
                raise ValueError('Nonfinite calculated output')
            return float(value)
        return value
    output = numbers(result)
    json.dumps(output, allow_nan=False)
    return output


def usd(value):
    return f'${money(D(str(value))):,.2f}'


def cost_section(data):
    s, c, u = data['codebase_stats'], data['cost_analysis'], data['ai_usage']
    t, a = c['traditional_development'], c['ai_assisted_development']
    rows = [('Production', s['backend_production'] + s['frontend_production'], '2.50', 'production'),
            ('Tests', s['backend_tests'] + s['frontend_tests'], '1.25', 'tests'),
            ('Configuration and shell scripts', s['infrastructure'], '1.00', 'infrastructure'),
            ('Documentation', s['documentation'], '0.75', 'documentation')]
    composition = '\n'.join(f'| {label} | {lines:,} | ${rate} | {usd(t["line_costs"][key])} |'
                            for label, lines, rate, key in rows)
    return f'''# Historical development cost model

<!-- Generated by scripts/update-cost-analysis.sh; standalone archival output. -->

This records an illustrative model using current physical line counts and fixed
historical staffing and usage assumptions. It is not an invoice, valuation,
replacement-cost measurement, or evidence of productivity gains or realized savings.
The numerical comparison in the accompanying JSON is a difference between modeled
amounts, not a measured counterfactual.

## Inputs and line-based estimate

Selected Git-tracked categories contain {s['total_lines']:,} lines. Physical lines
do not measure delivered functionality, quality, or manual effort.

| Category | Counted lines | Assumed USD per line | Modeled amount |
| --- | --- | --- | --- |
{composition}

| Component | Modeled amount |
| --- | --- |
| Base amount | {usd(t['base_cost'])} |
| Overhead assumption (75% of base) | {usd(t['overhead_cost'])} |
| Line-based total | {usd(t['total_cost'])} |

## Fixed historical baseline

| Input | Assumption | Amount |
| --- | --- | --- |
| Staff time | 224 hours at $150/hour | {usd(a['staff_engineer_cost'])} |
| Reported usage | Sum of the historical CSV Cost column | {usd(u['total_cost'])} |
| Subscription | One month | $20.00 |
| Baseline total | Staff time plus usage and subscription | {usd(a['total_cost'])} |

The historical export contains {u['total_requests']:,} records and
{u['total_tokens']:,} reported tokens, including cache reads and unsuccessful
requests. Reported usage cost before currency rounding is {u['total_cost']} USD.

## Limitations

The input export is data/usage-events-2025-09-26.csv. Its reported costs include
Included and Not Charged events; they are estimates, not verified invoices, and
may overlap subscription charges. Current line counts are compared with a fixed
historical baseline rather than matched projects or measured engineering effort.

Rates and the 75% overhead are assumptions retained from the original model.
Monetary components use round-half-up to cents and totals sum those components.
The category scope omits admin-frontend, non-shell scripts, and some tests outside
selected directories. Inline Rust tests remain in their file's category.
Tracked files are not guaranteed to be manually written.

This report and data/cost-analysis.json are the updater's only publication
outputs. Product documentation is not updated by this calculation.
'''


def render_documents(root, data):
    return {root / DOCUMENTS[0]: cost_section(data)}


def update(root, stats_path=None, usage_path=None):
    if stats_path:
        stats = json.loads(stats_path.read_text())
    else:
        stats = json.loads(subprocess.check_output(['bash', str(root / 'scripts/codebase-stats-corrected.sh'), '--json'], text=True))
    usage = read_usage(usage_path or root / 'data/usage-events-2025-09-26.csv')
    # Documentation itself is counted. Solve its final line counts in memory so
    # initial template migration cannot cause changes on the next invocation.
    for _ in range(5):
        data = calculate(stats, usage)
        rendered = render_documents(root, data)
        if stats_path:
            break
        final_stats = collect(root, {str(path.relative_to(root)): content.encode() for path, content in rendered.items()})
        if stats == final_stats:
            break
        stats = final_stats
    else:
        raise ValueError('Generated documentation statistics did not converge')
    # Prepare every document and serialize/parse the complete JSON before writing.
    target = root / 'data/cost-analysis.json'
    old = json.loads(target.read_text())
    old_values = {key: value for key, value in old.items() if key != 'last_updated'}
    data['last_updated'] = old.get('last_updated') if old_values == data else datetime.now(timezone.utc).strftime('%Y-%m-%dT%H:%M:%SZ')
    encoded = json.dumps(data, indent=2, allow_nan=False) + '\n'
    if json.loads(encoded) != data:
        raise ValueError('Generated JSON does not round-trip')
    rendered[target] = encoded
    changed = []
    for path, content in rendered.items():
        if not path.exists() or path.read_text() != content:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(content)
            changed.append(str(path.relative_to(root)))
    print(f"Validated {stats['total_lines']:,} lines; generated standalone historical model and JSON")
    print('Updated: ' + ', '.join(changed) if changed else 'No changes')
    return data


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--stats-json', type=Path, help='validated structured statistics fixture')
    parser.add_argument('--usage-csv', type=Path, help='usage export (defaults to repository data)')
    args = parser.parse_args()
    try:
        update(Path(__file__).resolve().parent.parent, args.stats_json, args.usage_csv)
    except (ValueError, ArithmeticError, OSError, csv.Error, subprocess.CalledProcessError) as error:
        print(f'Cost analysis failed: {error}', file=sys.stderr)
        return 1
    return 0


if __name__ == '__main__':
    sys.exit(main())


