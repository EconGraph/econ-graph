# Historical Development Data

This directory retains the historical usage export and machine-readable calculation
results for maintenance and reproducibility. These records are not product
positioning or evidence of development productivity.

## Inputs and output

- `usage-events-2025-09-26.csv`: historical usage events, including reported costs,
  token counts, event kinds, and timestamps. Reported usage estimates are not
  necessarily audited invoices.
- `cost-analysis.json`: generated codebase statistics, usage totals, and modeled
  cost calculations. Fixed staffing assumptions and historical usage coexist with
  current source counts; the model does not measure actual replacement cost.

The updater also writes a standalone archival report. It does not update the
README, investor pitch, product summary, or demo documentation.

## Maintenance

From the repository root:

```bash
./scripts/update-cost-analysis.sh
python3 -m unittest discover -s scripts/tests -p 'test_cost_analysis.py' -v
```

The shell entry point delegates to `scripts/cost_analysis.py`, which reads tracked
file statistics and validates CSV input before writing outputs. Repeated runs with
unchanged inputs preserve the generated contents and timestamp.

The scheduled workflow validates generation and proposes output changes through a
pull request. The legacy webhook entry point invokes the same updater; it does not
produce a separate summary.


