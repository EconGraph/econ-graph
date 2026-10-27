# Financial Analysis Configuration

These four JSON files hold concept mappings, benchmark percentiles, interpretation thresholds, and formula reference material for the optional financial-analysis library. Analysts can maintain this metadata without editing Rust. Implementing a new calculation or changing the concepts used in its arithmetic requires changes to the [financial ratio calculator](../src/financial_ratio_calculator.rs).

The loader and calculator require the off-by-default [`xbrl-parser` Cargo feature](../README.md#cargo-features). The default crawler-worker build leaves them out, and the live filing crawl does not use these files to calculate ratios.

## Loading configuration

[`FinancialAnalysisConfig::load_from_dir`](../src/config_loader.rs) reads and deserializes all four files from the directory supplied by its caller. `FinancialRatioCalculator::new` and `with_config` call this loader with `config`, relative to the process working directory. For example, that path finds these files when the process runs from `backend/crates/econ-graph-sec-crawler`. Use `FinancialRatioCalculator::with_config_dir` to supply a different directory together with a `RatioCalculationConfig`.

Missing files, invalid JSON, and missing required fields return errors to the caller; calculator construction fails if loading fails. Loading happens at construction. To apply edited files, construct a new calculator; an existing instance keeps its loaded configuration.

The examples below are complete minimal values for their respective files. The [loader's Rust types](../src/config_loader.rs) define the required fields. Empty maps are allowed, but required top-level fields must still be present.

## Configuration files

### `concept_mappings.json`

[Concept mappings](concept_mappings.json) associate standardized names such as `Assets` with XBRL taxonomy concept identifiers. Both `us_gaap` and `ifrs` maps are required:

```json
{
  "us_gaap": {
    "Assets": "us-gaap:Assets",
    "NetIncome": "us-gaap:NetIncomeLoss",
    "Revenue": "us-gaap:Revenues"
  },
  "ifrs": {}
}
```

`get_concept_mapping` accepts the taxonomy arguments `us-gaap` and `ifrs`; other values return `None`. Adding another top-level taxonomy does not extend this dispatch. The calculator's arithmetic selects concept identifiers in Rust and does not call its concept-mapping helper, so editing these maps alone does not change the concepts used in calculations.

### `ratio_benchmarks.json`

[Ratio benchmarks](ratio_benchmarks.json) store percentile values for ratios within an industry. The required `industries` map contains entries with `industry_code`, `industry_name`, and a `benchmarks` map. Each ratio entry requires `ratio_name` and all five percentile fields:

```json
{
  "industries": {
    "7370": {
      "industry_code": "7370",
      "industry_name": "Computer Programming, Data Processing, And Other Computer Related Services",
      "benchmarks": {
        "return_on_equity": {
          "ratio_name": "return_on_equity",
          "median": 0.15,
          "p25": 0.08,
          "p75": 0.25,
          "p90": 0.35,
          "p10": 0.03
        }
      }
    }
  }
}
```

`get_industry_benchmark("7370", "return_on_equity")` looks up the exact industry and ratio map keys. The shipped keys use SIC-style industry codes. SIC (Standard Industrial Classification) and GICS (Global Industry Classification Standard) are separate classification systems; the loader does not validate codes or translate between them.

The checked-in values are examples with no source or observation date recorded in the files. Treat them as illustrative, rather than sourced current industry statistics. The calculator does not use the benchmark lookup to populate results: its calculated ratios leave `benchmark_percentile` as `None`.

### `ratio_interpretations.json`

[Ratio interpretations](ratio_interpretations.json) map exact ratio names to descriptions selected by numeric thresholds. Each ratio requires all five levels, and every level requires `threshold` and `description`:

```json
{
  "return_on_equity": {
    "excellent": {
      "threshold": 0.20,
      "description": "Excellent - Strong profitability and efficient use of shareholder capital"
    },
    "good": {
      "threshold": 0.15,
      "description": "Good - Above average profitability"
    },
    "average": {
      "threshold": 0.10,
      "description": "Average - Moderate profitability"
    },
    "below_average": {
      "threshold": 0.05,
      "description": "Below average - Weak profitability"
    },
    "poor": {
      "threshold": 0.0,
      "description": "Poor - Very weak profitability"
    }
  }
}
```

`get_ratio_interpretation` checks `value >= threshold` in this order: `excellent`, `good`, `average`, then `below_average`. The first match supplies the description. If none matches, it returns the `poor` description without comparing the `poor` threshold. A missing ratio key returns `None`.

Use the same units as the calculator: for example, return on equity of 15% is `0.15`. The lookup applies the same comparison direction to every ratio, including ratios where lower values may be preferable. An ascending threshold sequence does not reverse the comparison. Interpretations have no company-size or industry dimension; adding those dimensions or a different comparison direction requires Rust changes.

### `ratio_formulas.json`

[Ratio formulas](ratio_formulas.json) provide readable formulas, explanations, and educational links. All seven category maps shown here are required, even when empty. Each ratio entry requires `formula`, `description`, and `educational_link`:

```json
{
  "profitability": {
    "return_on_equity": {
      "formula": "Net Income / Shareholders' Equity",
      "description": "Measures how efficiently a company uses shareholders' equity to generate profits",
      "educational_link": "https://www.investopedia.com/terms/r/returnonequity.asp"
    }
  },
  "liquidity": {},
  "leverage": {},
  "efficiency": {},
  "valuation": {},
  "warren_buffett_favorites": {},
  "growth": {}
}
```

`get_ratio_formula` searches these category maps by exact ratio key, and `get_ratios_by_category` lists the keys in a named category. New entries make reference material available through these APIs. Formula strings are descriptive text, not executable expressions; the calculator implements arithmetic and assigns its result formula strings in Rust. Editing this file does not change either behavior. A new category also requires changes to the loader's types and lookup methods.

## Maintaining the files

For a benchmark change, record the source, data date, peer-group definition, units, and calculation method in the change's review description or an accompanying reference document. These JSON types have no provenance fields. Cite the actual report, dataset, or database extract used; a provider name alone does not establish the origin of the numbers. For an interpretation or formula change, explain the rationale and confirm that the text matches the implemented calculation.

Validate JSON syntax, then load the full directory through `FinancialAnalysisConfig::load_from_dir` and fail the check on any returned error. Syntax checks alone do not detect missing required fields. For example, this program loads the checked-in files when run from the repository root, with `xbrl-parser` enabled in the consuming crate:

```rust
use econ_graph_sec_crawler::FinancialAnalysisConfig;

fn main() -> anyhow::Result<()> {
    let config = FinancialAnalysisConfig::load_from_dir(
        "backend/crates/econ-graph-sec-crawler/config",
    )?;
    assert!(config.get_ratio_formula("return_on_equity").is_some());
    Ok(())
}
```

Check the affected lookups with representative values, including values at and on either side of interpretation thresholds. Loading verifies the Rust schema; it does not establish the financial validity of benchmark values or threshold choices. For changes intended to affect calculations, check representative financial statements against the Rust implementation as well.

Keep changes in version control and have another analyst review the financial content. After review, make the edited directory available to the consuming application and construct a new calculator with that path. The library has no file-watching or reload mechanism.
