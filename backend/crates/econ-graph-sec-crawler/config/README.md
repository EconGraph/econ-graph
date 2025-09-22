# Financial Analysis Configuration

This directory contains configuration files that allow financial analysts to modify ratio calculations, benchmarks, and interpretations without changing code.

## Configuration Files

### `concept_mappings.json`
Maps standardized concept names to XBRL taxonomy concepts for different accounting standards.

**Structure:**
- `us_gaap`: US Generally Accepted Accounting Principles mappings
- `ifrs`: International Financial Reporting Standards mappings

**Example:**
```json
{
  "us_gaap": {
    "Assets": "us-gaap:Assets",
    "NetIncome": "us-gaap:NetIncomeLoss",
    "Revenue": "us-gaap:Revenues"
  }
}
```

**How to modify:**
- Add new concept mappings for different accounting standards
- Update existing mappings if XBRL taxonomy changes
- Add support for new taxonomies (e.g., industry-specific standards)

### `ratio_benchmarks.json`
Contains industry-specific benchmarks for financial ratios.

**Structure:**
- `industries`: Map of industry codes to benchmark data
- Each industry contains percentile benchmarks (p10, p25, median, p75, p90)

**Example:**
```json
{
  "industries": {
    "7370": {
      "industry_code": "7370",
      "industry_name": "Computer Programming, Data Processing, And Other Computer Related Services",
      "benchmarks": {
        "return_on_equity": {
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

**How to modify:**
- Add new industries using SIC or GICS codes
- Update benchmark percentiles based on latest industry data
- Add new ratio types for industry-specific analysis
- Source data from industry reports, academic studies, or financial databases

### `ratio_interpretations.json`
Defines interpretation thresholds and descriptions for financial ratios.

**Structure:**
- Each ratio has five interpretation levels: excellent, good, average, below_average, poor
- Each level has a threshold value and descriptive text

**Example:**
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
    }
  }
}
```

**How to modify:**
- Adjust thresholds based on market conditions or industry standards
- Update descriptions to reflect current market expectations
- Add new ratios with their interpretation criteria
- Consider different thresholds for different company sizes or industries

### `ratio_formulas.json`
Contains formulas, descriptions, and educational links for financial ratios.

**Structure:**
- Organized by category: profitability, liquidity, leverage, efficiency, valuation, warren_buffett_favorites, growth
- Each ratio includes formula, description, and educational link

**Example:**
```json
{
  "profitability": {
    "return_on_equity": {
      "formula": "Net Income / Shareholders' Equity",
      "description": "Measures how efficiently a company uses shareholders' equity to generate profits",
      "educational_link": "https://www.investopedia.com/terms/r/returnonequity.asp"
    }
  }
}
```

**How to modify:**
- Add new ratios with their formulas and explanations
- Update educational links to point to current resources
- Add industry-specific or custom ratios
- Include alternative calculation methods for the same ratio

## Best Practices for Financial Analysts

### 1. Benchmark Updates
- Update benchmarks quarterly or annually based on latest industry data
- Consider seasonal variations in certain industries
- Use multiple data sources to ensure accuracy
- Document the source and date of benchmark data

### 2. Threshold Adjustments
- Review interpretation thresholds annually
- Consider market conditions (bull vs. bear markets)
- Adjust for inflation and interest rate changes
- Test thresholds against historical data

### 3. New Ratio Development
- Start with clear business rationale for the new ratio
- Define calculation methodology precisely
- Set appropriate interpretation thresholds
- Include educational resources for users

### 4. Quality Assurance
- Validate JSON syntax before deploying changes
- Test new configurations with sample data
- Document changes and their rationale
- Maintain version control of configuration files

## Data Sources for Benchmarks

### Industry Data
- **SIC/GICS Codes**: Standard Industrial Classification codes
- **Industry Reports**: McKinsey, Deloitte, PwC industry studies
- **Academic Research**: Financial journals and studies
- **Government Data**: SEC, Federal Reserve, Bureau of Economic Analysis

### Financial Databases
- **Bloomberg Terminal**: Industry ratio databases
- **S&P Capital IQ**: Industry benchmarking tools
- **FactSet**: Peer analysis and benchmarking
- **Morningstar**: Industry and sector analysis

### Professional Organizations
- **CFA Institute**: Financial analysis standards
- **AICPA**: Accounting and auditing standards
- **Industry Associations**: Sector-specific benchmarks

## Version Control

All configuration changes should be:
1. **Documented**: Include change rationale and data sources
2. **Tested**: Validate with sample financial statements
3. **Reviewed**: Have another analyst review changes
4. **Versioned**: Use git to track configuration history

## Emergency Updates

For urgent market condition changes:
1. Create a hotfix branch
2. Update only the necessary thresholds
3. Test with current data
4. Deploy and document the emergency change
5. Follow up with a proper review and permanent update

## Contact

For questions about configuration changes or to suggest new ratios:
- **Technical Issues**: Backend development team
- **Financial Content**: Senior financial analysts
- **Data Sources**: Research team
