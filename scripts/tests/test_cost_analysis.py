"""Regression tests for structured inputs, arithmetic, and full updater behavior."""
import csv
from decimal import Decimal
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import cost_analysis as cost
from codebase_stats import collect

ROOT = Path(__file__).resolve().parents[2]


def stats():
    return dict(backend_production=100, backend_tests=40, frontend_production=200,
                frontend_tests=60, configuration=20, documentation=80, scripts=30,
                total_lines=530)


def usage():
    return dict(total_cost=Decimal('12.345'), total_tokens=10, total_requests=1)


class CostTests(unittest.TestCase):
    def test_known_cost_model_and_rounding(self):
        result = cost.calculate(stats(), usage())
        traditional = result['cost_analysis']['traditional_development']
        self.assertEqual(traditional['line_costs'], dict(production=750, tests=125, infrastructure=50, documentation=60))
        self.assertEqual(traditional['base_cost'], 985)
        self.assertEqual(traditional['overhead_cost'], 738.75)
        self.assertEqual(traditional['total_cost'], 1723.75)
        self.assertEqual(result['cost_analysis']['ai_assisted_development']['ai_tool_cost'], 32.35)
        self.assertEqual(result['cost_analysis']['savings'], dict(dollar_amount=-31908.6, percentage=-1851.1))
        self.assertEqual(result['codebase_stats']['infrastructure'], 50)
        self.assertEqual(result['ai_usage']['total_cost'], 12.345)

    def test_invalid_statistics(self):
        cases = []
        missing = stats(); missing.pop('scripts'); cases.append(missing)
        for value in ['100', '1,000', '\x1b[32m100\x1b[0m', None, True, -1, 1.5]:
            malformed = stats(); malformed['backend_production'] = value; cases.append(malformed)
        inconsistent = stats(); inconsistent['total_lines'] = 0; cases.append(inconsistent)
        cases.append(dict.fromkeys(stats(), 0))
        for case in cases:
            with self.subTest(stats=case), self.assertRaises(ValueError):
                cost.calculate(case, usage())

    def test_invalid_usage_totals(self):
        for amount in [Decimal('NaN'), Decimal('Infinity'), Decimal('-1'), 1.0]:
            with self.subTest(amount=amount), self.assertRaises(ValueError):
                cost.calculate(stats(), {**usage(), 'total_cost': amount})

    def csv_file(self, directory, **changes):
        path = Path(directory) / 'usage.csv'
        row = dict(Date='2025-09-26', Kind='Errored, Not Charged', Model='auto', **{'Max Mode': 'No',
                   'Input (w/ Cache Write)': '1', 'Input (w/o Cache Write)': '2', 'Cache Read': '3',
                   'Output Tokens': '4', 'Total Tokens': '10', 'Cost': '12.345'})
        row.update(changes)
        with path.open('w', newline='') as output:
            writer = csv.DictWriter(output, fieldnames=row)
            writer.writeheader(); writer.writerow(row)
        return path

    def test_quoted_csv_and_cost_over_ten(self):
        with tempfile.TemporaryDirectory() as directory:
            result = cost.read_usage(self.csv_file(directory))
        self.assertEqual(result, usage())

    def test_invalid_csv(self):
        for changes in [{'Cost': ''}, {'Cost': 'NaN'}, {'Cost': '-2'}, {'Cost': 'oops'},
                        {'Total Tokens': '9'}, {'Cache Read': '3.5'}]:
            with self.subTest(changes=changes), tempfile.TemporaryDirectory() as directory:
                with self.assertRaises(ValueError):
                    cost.read_usage(self.csv_file(directory, **changes))
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'bad.csv'
            for content in ['Date,Cost\n2025,1\n', 'Cost,Cost\n1,2\n', '', '"unclosed']:
                path.write_text(content)
                with self.assertRaises((ValueError, csv.Error)):
                    cost.read_usage(path)
            valid = self.csv_file(directory).read_text()
            path.write_text(valid.splitlines()[0] + '\n')
            with self.assertRaises(ValueError):
                cost.read_usage(path)
            path.write_text(valid.rstrip() + ',extra\n')
            with self.assertRaises(ValueError):
                cost.read_usage(path)

    def fixture(self, directory):
        root = Path(directory)
        for name in [*cost.DOCUMENTS, 'data/cost-analysis.json']:
            target = root / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes((ROOT / name).read_bytes())
        csv_path = self.csv_file(directory)
        stats_path = root / 'stats.json'
        stats_path.write_text(json.dumps(stats()))
        return root, stats_path, csv_path

    def test_repeatable_docs_and_json_with_changed_inputs(self):
        with tempfile.TemporaryDirectory() as directory, patch('sys.stdout', new=io.StringIO()):
            root, stats_path, csv_path = self.fixture(directory)
            first = cost.update(root, stats_path, csv_path)
            before = {p: p.read_bytes() for p in root.rglob('*') if p.is_file()}
            self.assertEqual(first, cost.update(root, stats_path, csv_path))
            self.assertEqual(before, {p: p.read_bytes() for p in before})
            changed = stats(); changed['backend_production'] += 100; changed['total_lines'] += 100
            stats_path.write_text(json.dumps(changed))
            updated = cost.update(root, stats_path, csv_path)
            self.assertEqual(updated['codebase_stats']['total_lines'], 630)
            for name in cost.DOCUMENTS:
                self.assertIn('630', (root / name).read_text())
            self.assertFalse(list(root.rglob('*.bak')))

    def test_initial_migration_converges_and_second_run_is_byte_identical(self):
        with tempfile.TemporaryDirectory() as directory, patch('sys.stdout', new=io.StringIO()):
            root, _, csv_path = self.fixture(directory)
            # Include the legacy zero-total/corrupt-percentage document and old
            # literal replacement targets, then track them for real statistics.
            document = root / 'docs/business/COST_ASSUMPTIONS_AND_PRODUCTIVITY_ANALYSIS.md'
            document.write_text('## EconGraph Project Cost Analysis\n\n'
                                '**Total Codebase**: 0 lines\n'
                                '#### Production Code (72610 lines - %)\n\n'
                                '## Productivity Multipliers\n\n'
                                '## Conclusion\n(92.4% cost reduction)\n'
                                '1. **Traditional Development**: $455142.18\n'
                                '2. **AI-Assisted Development**: ~$34557.836\n')
            subprocess.run(['git', 'init', '-q', directory], check=True)
            subprocess.run(['git', '-C', directory, 'add', '.'], check=True)
            actual_output = subprocess.check_output
            def structured(args, **kwargs):
                if args[0] == 'bash':
                    return json.dumps(collect(root), sort_keys=True)
                return actual_output(args, **kwargs)
            # Exercise the actual collector without replacing its Git/file logic.
            with patch('cost_analysis.subprocess.check_output', side_effect=structured):
                first = cost.update(root, usage_path=csv_path)
            before = {p: p.read_bytes() for p in root.rglob('*') if p.is_file()}
            self.assertEqual(first['codebase_stats']['total_lines'], collect(root)['total_lines'])
            with patch('cost_analysis.subprocess.check_output', side_effect=structured):
                self.assertEqual(first, cost.update(root, usage_path=csv_path))
            self.assertEqual(before, {p: p.read_bytes() for p in before})

    def test_failure_before_any_write(self):
        with tempfile.TemporaryDirectory() as directory:
            root, stats_path, csv_path = self.fixture(directory)
            originals = {p: p.read_bytes() for p in root.rglob('*') if p.is_file()}
            with patch('cost_analysis.render_documents', side_effect=ArithmeticError('divide by zero')):
                with self.assertRaises(ArithmeticError):
                    cost.update(root, stats_path, csv_path)
            self.assertEqual(originals, {p: p.read_bytes() for p in originals})
            (root / cost.DOCUMENTS[-1]).write_text('missing anchor')
            originals = {p: p.read_bytes() for p in root.rglob('*') if p.is_file()}
            with self.assertRaises(ValueError):
                cost.update(root, stats_path, csv_path)
            self.assertEqual(originals, {p: p.read_bytes() for p in originals})

    def test_invalid_and_zero_inputs_fail_shell_command_without_writes(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'stats.json'
            originals = {name: (ROOT / name).read_bytes() for name in [*cost.DOCUMENTS, 'data/cost-analysis.json']}
            for content in ['{', '{}', json.dumps(dict.fromkeys(stats(), 0)), json.dumps({**stats(), 'total_lines': 0})]:
                path.write_text(content)
                result = subprocess.run(['bash', str(ROOT / 'scripts/update-cost-analysis.sh'), '--stats-json', str(path)], capture_output=True)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(b'Cost analysis failed:', result.stderr)
            self.assertEqual(originals, {name: (ROOT / name).read_bytes() for name in originals})

    def test_structured_stats_ignore_ansi_locale_and_handle_spaces(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            subprocess.run(['git', 'init', '-q', directory], check=True)
            files = {'backend/crates/src/main.rs': 1234, 'backend/crates/tests/a.rs': 2,
                     'frontend/src/space name.ts': 3456, 'README.md': 4,
                     'data/cost-analysis.json': 100, 'package-lock.json': 100}
            for name, lines in files.items():
                path = root / name; path.parent.mkdir(parents=True, exist_ok=True); path.write_text('x\n' * lines)
            subprocess.run(['git', '-C', directory, 'add', '.'], check=True)
            result = collect(root)
            self.assertEqual(result['total_lines'], 4696)
            self.assertEqual(result['backend_production'], 1234)
            self.assertEqual(result['frontend_production'], 3456)
            cost.validate_stats(json.loads(json.dumps(result)))
            (root / 'README.md').unlink()
            with self.assertRaises(OSError):
                collect(root)
        with tempfile.TemporaryDirectory() as directory:
            subprocess.run(['git', 'init', '-q', directory], check=True)
            empty = collect(Path(directory))
            self.assertEqual(empty['total_lines'], 0)
            with self.assertRaises(ValueError):
                cost.validate_stats(empty)


if __name__ == '__main__':
    unittest.main()
