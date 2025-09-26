# EconGraph Cost Analysis Data

This directory contains the source data and generated files for EconGraph's cost analysis system.

## 📊 Source Data Files

### `usage-events-2025-09-26.csv`
- **Description**: Actual AI usage data from Cursor
- **Contains**: Token usage, costs, timestamps
- **Updated**: When new usage data is available
- **Format**: CSV with columns for cost, tokens, timestamps

### `cost-analysis.json`
- **Description**: Generated cost analysis data
- **Contains**: Current cost figures, codebase stats, ROI calculations
- **Updated**: Automatically by update scripts
- **Format**: JSON with structured cost data

### `cost-summary.md`
- **Description**: Human-readable cost summary
- **Contains**: Formatted cost analysis results
- **Updated**: Automatically by webhook system
- **Format**: Markdown summary

## 🔄 Auto-Update System

### Update Scripts

#### `../scripts/update-cost-analysis.sh`
- **Purpose**: Main cost analysis update script
- **Triggers**: Manual execution or GitHub Actions
- **Updates**: All cost figures in documentation
- **Dependencies**: `codebase-stats-corrected.sh`, usage CSV

#### `../scripts/webhook-cost-update.py`
- **Purpose**: Webhook handler for real-time updates
- **Triggers**: External webhooks or API calls
- **Updates**: Cost analysis and generates summary
- **Dependencies**: Update script, cost data JSON

### GitHub Actions

#### `.github/workflows/update-cost-analysis.yml`
- **Schedule**: Daily at 6 AM UTC
- **Triggers**: 
  - Scheduled runs
  - Manual dispatch
  - Changes to source data files
- **Actions**:
  - Updates cost analysis
  - Commits changes
  - Creates summary

## 📈 Cost Tracking

### Key Metrics Tracked

1. **Codebase Statistics**
   - Total lines of code
   - Production vs test vs infrastructure vs documentation
   - Growth over time

2. **Cost Analysis**
   - Traditional development costs
   - AI-assisted development costs
   - Cost savings and ROI
   - Cost per line of code

3. **AI Usage**
   - Token consumption
   - Actual costs
   - Usage patterns

### Data Flow

```
Source Data → Update Script → Cost Analysis → Documentation
     ↓              ↓              ↓              ↓
Usage CSV → Calculate Costs → JSON Data → Markdown Docs
Codebase → Update Figures → ROI Analysis → Business Docs
```

## 🛠️ Usage

### Manual Update
```bash
# Run the update script
./scripts/update-cost-analysis.sh

# Check the results
cat data/cost-analysis.json
cat data/cost-summary.md
```

### Webhook Update
```bash
# Trigger webhook update
python3 scripts/webhook-cost-update.py
```

### GitHub Actions
- Automatic daily updates
- Manual trigger via GitHub UI
- Automatic trigger on data file changes

## 📋 Data Schema

### `cost-analysis.json` Structure
```json
{
  "last_updated": "2025-09-26T12:00:00Z",
  "codebase_stats": {
    "total_lines": 159335,
    "backend_production": 45200,
    "frontend_production": 27410,
    "backend_tests": 4200,
    "frontend_tests": 3257,
    "infrastructure": 43068,
    "documentation": 35200,
    "scripts": 31618
  },
  "cost_analysis": {
    "traditional_development": {
      "base_cost": 260314,
      "overhead_cost": 195235,
      "total_cost": 455549
    },
    "ai_assisted_development": {
      "staff_engineer_cost": 33600,
      "ai_tool_cost": 957,
      "total_cost": 34557
    },
    "savings": {
      "dollar_amount": 420992,
      "percentage": 92.4
    }
  },
  "ai_usage": {
    "total_cost": 937.84,
    "total_tokens": 3250000000
  }
}
```

## 🔗 Links to Documentation

- **Cost Assumptions**: [../docs/business/COST_ASSUMPTIONS_AND_PRODUCTIVITY_ANALYSIS.md](../docs/business/COST_ASSUMPTIONS_AND_PRODUCTIVITY_ANALYSIS.md)
- **Product Summary**: [../docs/business/PRODUCT_SUMMARY_2025.md](../docs/business/PRODUCT_SUMMARY_2025.md)
- **Investor Pitch**: [../docs/business/INVESTOR_PITCH.md](../docs/business/INVESTOR_PITCH.md)
- **Main README**: [../README.md](../README.md)

---

*This directory is automatically maintained by the EconGraph cost analysis system.*
