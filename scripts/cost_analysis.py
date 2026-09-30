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
DOCUMENTS = ['README.md', 'docs/business/COST_ASSUMPTIONS_AND_PRODUCTIVITY_ANALYSIS.md',
             'docs/business/PRODUCT_SUMMARY_2025.md', 'docs/business/INVESTOR_PITCH.md',
             'demo-tools/PROFESSIONAL_DEMO_SUMMARY.md']


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


def substitute(text, pattern, replacement, minimum=1):
    text, count = re.subn(pattern, lambda _: replacement, text, flags=re.MULTILINE)
    if count < minimum:
        raise ValueError(f'Documentation anchor missing: {pattern}')
    return text


def cost_section(data):
    s, c, u = data['codebase_stats'], data['cost_analysis'], data['ai_usage']
    t, a, savings = c['traditional_development'], c['ai_assisted_development'], c['savings']
    prod = s['backend_production'] + s['frontend_production']
    tests = s['backend_tests'] + s['frontend_tests']
    rows = [('Production Code', prod, '2.50', 'production'), ('Test Code', tests, '1.25', 'tests'),
            ('Infrastructure (configuration + scripts)', s['infrastructure'], '1.00', 'infrastructure'),
            ('Documentation', s['documentation'], '0.75', 'documentation')]
    composition = '\n'.join(f'| {label} | {lines:,} | {lines * 100 / s["total_lines"]:.1f}% | ${rate} | {usd(t["line_costs"][key])} |'
                            for label, lines, rate, key in rows)
    return f'''## EconGraph Project Cost Analysis

<!-- cost-analysis: generated; run scripts/update-cost-analysis.sh -->
All monetary estimates below are in USD. Line counts are physical newline counts
in the existing Git-tracked categories, not a measure of delivered functionality.

### Codebase Composition and Traditional Cost

**Total Codebase**: {s['total_lines']:,} lines of manually written code (selected Git-tracked categories; excludes lock files and generated cost JSON)

| Code Type | Lines | Share | Rate/Line | Base Cost |
|-----------|-------|-------|-----------|-----------|
{composition}
| **Total Base Cost** | **{s['total_lines']:,}** | **100.0%** | | **{usd(t['base_cost'])}** |

- **Backend Production**: {s['backend_production']:,} lines
- **Backend Tests**: {s['backend_tests']:,} lines
- **Frontend Production**: {s['frontend_production']:,} lines
- **Frontend Tests**: {s['frontend_tests']:,} lines
- **Configuration Files**: {s['configuration']:,} lines
- **Scripts and Automation**: {s['scripts']:,} lines

| Cost Category | Amount |
|---------------|--------|
| Base Development Cost | {usd(t['base_cost'])} |
| Overhead (75% of base) | {usd(t['overhead_cost'])} |
| **Total Traditional Cost** | **{usd(t['total_cost'])}** |

Overhead preserves the original 20% project management + 15% code reviews +
25% testing/QA + 15% integration/deployment assumptions. Monetary components
are rounded to cents with round-half-up; total cost is the sum of those components.
Shares are rounded independently and may not sum to exactly 100.0%.

### AI-Assisted Development Cost

| Component | Assumption | Amount |
|-----------|------------|--------|
| Staff engineer | 224 hours × $150/hour | {usd(a['staff_engineer_cost'])} |
| Reported token usage | Sum of Cost column, rounded once | {usd(u['total_cost'])} |
| Cursor Pro | One month | $20.00 |
| **Total AI Tool Costs** | | **{usd(a['ai_tool_cost'])}** |
| **Total AI-Assisted Cost** | | **{usd(a['total_cost'])}** |

- **Total AI Interactions**: {u['total_requests']:,} CSV records
- **Total Tokens Processed**: {u['total_tokens']:,} tokens (including cache reads and unsuccessful requests)
- **Reported CSV Cost**: {u['total_cost']} USD before currency rounding
- **Daily Average**: {usd(a['total_cost'] / 28)} over the assumed 28 days

### Cost Comparison

| Development Approach | Total Cost | Cost per Counted Line |
|----------------------|------------|-----------------------|
| Traditional Development | {usd(t['total_cost'])} | {usd(t['total_cost'] / s['total_lines'])} |
| AI-Assisted Development | {usd(a['total_cost'])} | {usd(a['total_cost'] / s['total_lines'])} |
| **Savings** | **{usd(savings['dollar_amount'])}** | **{savings['percentage']:.1f}% reduction** |

### Assumptions and Limits

Staff rate reference: [Geomotiv](https://geomotiv.com/blog/software-engineer-hourly-rate-in-the-usa/) (historical assumption, not an invoice).
These estimates preserve the updater's USD/line rates, 75% overhead, 224 staff
hours, and one $20 subscription month. They are not measured replacement costs.
The 2025 CSV's reported costs include Included and Not Charged events; they are
not verified invoices and may overlap subscription charges. Current line counts
are compared with this fixed historical usage/staffing baseline.
The original category scope omits admin-frontend, non-shell scripts, and some
tests outside the selected directories; inline Rust tests remain in their file's
category. Tracked files are not guaranteed to be manually written.
Timeline, quality, productivity, and feature-count claims are independent
assumptions, not outputs of these calculations.
<!-- /cost-analysis -->

'''


def render_documents(root, data):
    s, c, u = data['codebase_stats'], data['cost_analysis'], data['ai_usage']
    total = f'{s["total_lines"]:,}'
    production = s['backend_production'] + s['frontend_production']
    tests = s['backend_tests'] + s['frontend_tests']
    traditional = usd(c['traditional_development']['total_cost'])
    ai = usd(c['ai_assisted_development']['total_cost'])
    percentage = f'{c["savings"]["percentage"]:.1f}'
    rendered = {}
    for name in DOCUMENTS:
        text = (root / name).read_text()
        original_lines = set(text.split("\n"))
        if 'COST_ASSUMPTIONS' in name:
            text = substitute(text, r'## EconGraph Project Cost Analysis\n[\s\S]*?(?=## Productivity Multipliers)', cost_section(data))
            text = substitute(text, r'\(-?[\d.]+% cost reduction\)', f'({percentage}% cost reduction)')
            text = substitute(text, r'1\. \*\*Traditional Development\*\*: .*', f'1. **Traditional Development**: {traditional} under the line-based model')
            text = substitute(text, r'2\. \*\*AI-Assisted Development\*\*: .*', f'2. **AI-Assisted Development**: {ai} under the fixed staffing/usage assumptions')
        elif name == 'README.md':
            text = substitute(text, r'### 📊 \*\*Cursor AI Usage Statistics \([^\n]+\)\*\*', '### 📊 **Cursor AI Usage Statistics (historical CSV export)**')
            text = substitute(text, r'- \*\*Total AI Interactions\*\*: .*', f'- **Total AI Interactions**: {u["total_requests"]:,} CSV records')
            text = substitute(text, r'- \*\*Total Tokens Processed\*\*: .*', f'- **Total Tokens Processed**: **{u["total_tokens"]:,} tokens** (reported usage, including cached and unsuccessful requests)')
            # Historical success, average size, and peak-day claims cannot reconcile
            # with this complete export. Avoid presenting them as current CSV metrics.
            text = substitute(text, r'- \*\*(?:Success Rate|Usage Scope)\*\*: .*', '- **Usage Scope**: Includes Included, Errored, Not Charged, and Aborted, Not Charged events')
            text = substitute(text, r'- \*\*Average Request Size\*\*: .*', f'- **Average Request Size**: {u["total_tokens"] / u["total_requests"]:,.0f} tokens')
            text = substitute(text, r'- \*\*(?:Peak Development Day|Usage Source)\*\*: .*', '- **Usage Source**: `data/usage-events-2025-09-26.csv` (historical export)')
            text = substitute(text, r'- \*\*Actual Token Costs\*\*: .*', f'- **Actual Token Costs**: {usd(u["total_cost"])} reported CSV estimate (not a verified bill)')
            text = substitute(text, r'- \*\*Total AI-Assisted Cost\*\*: .*', f'- **Total AI-Assisted Cost**: {ai} (fixed staffing and subscription assumptions)')
            text = substitute(text, r'- \*\*Daily Average\*\*: .*', f'- **Daily Average**: {usd(c["ai_assisted_development"]["total_cost"] / 28)} over the assumed 28 days')
            text = substitute(text, r'- \*\*Cost per Major Feature\*\*: .*', f'- **Cost per Major Feature**: {usd(c["ai_assisted_development"]["total_cost"] / 15)} assuming 15 features')
            text = substitute(text, r'- \*\*📝 Lines of Code\*\*: .*', f'- **📝 Lines of Code**: {total} total ({production:,} production code, {tests:,} test code, {s["infrastructure"]:,} configuration/scripts, {s["documentation"]:,} documentation)')
            text = substitute(text, r'The ~?\$[\d,.]+ (?:total investment produced a full-stack application that would typically require \$[\d,.]+ in development costs|estimated investment produced a full-stack application with a line-based traditional development estimate of \$[\d,.]+)', f'The {ai} estimated investment produced a full-stack application with a line-based traditional development estimate of {traditional}')
        elif 'PRODUCT_SUMMARY' in name:
            for label, value in [('Total Codebase', f'{total} lines of code'), ('Production Code', f'{production:,} lines (Rust backend, React frontend)'),
                                 ('Test Coverage', f'{tests:,} lines in selected test files (not coverage percentage)'),
                                 ('Infrastructure', f'{s["infrastructure"]:,} lines of configuration and scripts; {s["documentation"]:,} documentation lines'),
                                 ('Development Cost', f'{ai} (AI-assisted estimate) vs {traditional} (traditional estimate)'),
                                 ('Traditional Development', traditional), ('AI-Assisted Development', ai), ('Cost Savings', f'{percentage}% reduction')]:
                text = substitute(text, rf'- \*\*{label}\*\*: ' + (r'[\d,]+ lines.*' if label in {'Test Coverage', 'Infrastructure'} else r'.*'), f'- **{label}**: {value}')
            text = substitute(text, r'- \*\*Development\*\*: AI-assisted .*', f'- **Development**: AI-assisted model estimates a {percentage}% cost reduction')
            # Match the current generated wording as well as the legacy claim.
            text = substitute(text, r'With [\d,]+ lines of code delivered at ~?\$[\d,.]+ vs \$[\d,.]+(?:M)? traditional cost', f'With {total} lines of code delivered at {ai} vs {traditional} traditional cost')
        elif 'INVESTOR' in name:
            text = substitute(text, r'- \*\*Comprehensive Testing\*\*: .*', f'- **Comprehensive Testing**: {tests:,} counted lines in selected test files')
            text = substitute(text, r'- \*\*AI-Assisted Development\*\*: .*', f'- **AI-Assisted Development**: {total} counted lines; {ai} AI-assisted vs {traditional} traditional estimate ({percentage}% modeled savings)')
        else:
            text = substitute(text, r'- \*\*[\d,]+ Lines of Code\*\*: .*', f'- **{total} Lines of Code**: Selected production, test, configuration, script, and documentation files')
            text = substitute(text, r'- \*\*AI-Assisted Development\*\*: .*', f'- **AI-Assisted Development**: {ai} total estimate vs {traditional} traditional estimate')
        # Keep touched Markdown free of trailing whitespace.
        rendered[root / name] = '\n'.join(line if line in original_lines else line.rstrip() for line in text.split('\n'))
    return rendered


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
        if path.read_text() != content:
            path.write_text(content)
            changed.append(str(path.relative_to(root)))
    print(f"Validated {stats['total_lines']:,} lines; traditional {usd(data['cost_analysis']['traditional_development']['total_cost'])}; AI {usd(data['cost_analysis']['ai_assisted_development']['total_cost'])}")
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
