# Cost Analysis Automation System

> **CI/CD Documentation**: Comprehensive guide to the automated cost analysis system that keeps business documentation synchronized with source data.

## 🎯 Overview

The EconGraph project includes an automated cost analysis system that continuously updates business documentation with accurate cost figures based on real source data. This system ensures transparency and accuracy in cost reporting while reducing manual maintenance overhead.

## 🏗️ System Architecture

### Data Flow
```
Source Data → Update Script → Cost Analysis → Documentation
     ↓              ↓              ↓              ↓
Usage CSV → Calculate Costs → JSON Data → Markdown Docs
Codebase → Update Figures → ROI Analysis → Business Docs
```

### Components

1. **Source Data Layer**
   - `data/usage-events-2025-09-26.csv` - AI usage data
   - `scripts/codebase-stats-corrected.sh` - Codebase statistics (only stats script)
   - Real-time codebase changes

2. **Processing Layer**
   - `scripts/update-cost-analysis.sh` - Main update script
   - `scripts/webhook-cost-update.py` - Webhook handler
   - Cost calculation algorithms

3. **Output Layer**
   - `data/cost-analysis.json` - Structured cost data
   - Updated business documentation
   - Cost summaries and reports

4. **Automation Layer**
   - GitHub Actions workflow
   - Scheduled updates
   - Manual triggers

## 🔄 GitHub Actions Workflow

### Workflow File: `.github/workflows/update-cost-analysis.yml`

#### Triggers
```yaml
on:
  schedule:
    # Daily at 6 AM UTC
    - cron: '0 6 * * *'
  workflow_dispatch:
    # Manual trigger via GitHub UI
  push:
    paths:
      - 'data/usage-events-*.csv'
      - 'scripts/codebase-stats-corrected.sh'
      - 'backend/src/**'
      - 'frontend/src/**'
```

#### Workflow Steps

1. **Checkout Repository**
   - Uses `actions/checkout@v4`
   - Provides access to source code and data files

2. **Setup Dependencies**
   - Node.js 18 for any JavaScript dependencies
   - Rust toolchain for codebase analysis
   - System packages (bc for calculations)

3. **Update Cost Analysis**
   - Runs the main update script
   - Extracts current codebase statistics
   - Calculates updated cost figures
   - Updates all documentation

4. **Check for Changes**
   - Detects if any files were modified
   - Only commits if changes are present

5. **Commit and Push**
   - Commits changes with descriptive message
   - Pushes to repository
   - Creates summary of changes

#### Security Considerations
- Uses `GITHUB_TOKEN` for authentication
- No external secrets required
- All data processing happens in GitHub Actions environment

## 📊 Update Script Documentation

### Main Script: `scripts/update-cost-analysis.sh`

#### Purpose
Automatically updates cost figures in documentation based on source data.

#### Dependencies
- `scripts/codebase-stats-corrected.sh` - Codebase statistics
- `data/usage-events-2025-09-26.csv` - AI usage data
- `bc` - Mathematical calculations
- Standard Unix tools (grep, awk, sed)

#### Functions

##### `get_codebase_stats()`
- Runs the codebase statistics script
- Extracts line counts for different code categories
- Parses output to get current metrics
- Handles errors if stats script is missing

##### `get_ai_costs()`
- Reads AI usage data from CSV file
- Extracts total cost and token usage
- Falls back to default values if CSV missing
- Provides actual usage data for calculations

##### `calculate_costs()`
- Calculates traditional development costs
- Computes AI-assisted development costs
- Determines cost savings and ROI
- Uses industry-standard rates and formulas

##### `update_documentation()`
- Updates all business documentation files
- Replaces cost figures with latest calculations
- Maintains consistency across all documents
- Preserves document formatting and structure

##### `create_cost_data()`
- Creates structured JSON data file
- Includes all cost calculations and metrics
- Provides machine-readable cost data
- Enables programmatic access to cost figures

#### Error Handling
- Validates input files exist
- Checks for required dependencies
- Provides clear error messages
- Exits gracefully on failures

#### Output
- Updates all documentation files
- Creates `data/cost-analysis.json`
- Provides summary of changes
- Logs all operations

## 🤖 Webhook System

### Webhook Handler: `scripts/webhook-cost-update.py`

#### Purpose
Provides real-time cost analysis updates via webhook triggers.

#### Features
- Python-based webhook handler
- JSON data structure output
- Error handling and logging
- Summary generation

#### Usage
```bash
# Manual trigger
python3 scripts/webhook-cost-update.py

# Via webhook (external trigger)
curl -X POST https://your-domain.com/webhook/cost-update
```

#### Output Files
- `data/cost-analysis.json` - Structured cost data
- `data/cost-summary.md` - Human-readable summary

## 📁 Data Directory Structure

### `data/` Directory
```
data/
├── README.md                    # Data directory documentation
├── usage-events-2025-09-26.csv # AI usage data (source)
├── cost-analysis.json          # Generated cost data
└── cost-summary.md             # Generated summary
```

### Data Schema

#### `cost-analysis.json`
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

## 🔧 Maintenance and Troubleshooting

### Common Issues

#### 1. Missing Dependencies
**Problem**: Script fails with "command not found" errors
**Solution**: Install required packages
```bash
# Ubuntu/Debian
sudo apt-get install bc

# macOS
brew install bc
```

#### 2. CSV File Not Found
**Problem**: AI usage data missing
**Solution**: Ensure CSV file is in data directory
```bash
# Check if file exists
ls -la data/usage-events-*.csv

# If missing, add placeholder or update script
```

#### 3. Codebase Stats Script Fails
**Problem**: Statistics extraction fails
**Solution**: Check script permissions and dependencies
```bash
# Make script executable
chmod +x scripts/codebase-stats-corrected.sh

# Test manually
./scripts/codebase-stats-corrected.sh
```

#### 4. Documentation Update Failures
**Problem**: sed commands fail to update files
**Solution**: Check file permissions and backup files
```bash
# Remove backup files
find . -name "*.bak" -delete

# Check file permissions
ls -la docs/business/
```

### Monitoring

#### GitHub Actions Logs
- Check workflow runs in GitHub Actions tab
- Review logs for error messages
- Monitor execution time and resource usage

#### Manual Testing
```bash
# Test the update script
./scripts/update-cost-analysis.sh

# Check output files
cat data/cost-analysis.json
cat data/cost-summary.md

# Verify documentation updates
grep -r "Total Cost" docs/business/
```

### Performance Considerations

#### Script Execution Time
- Typical runtime: 30-60 seconds
- Depends on codebase size and complexity
- GitHub Actions timeout: 6 hours (plenty of headroom)

#### Resource Usage
- Minimal CPU and memory requirements
- No external API calls
- Local file processing only

## 🚀 Future Enhancements

### Planned Features

1. **Real-time Updates**
   - Webhook integration with external services
   - API endpoints for cost data access
   - Real-time dashboard updates

2. **Enhanced Analytics**
   - Historical cost tracking
   - Trend analysis and forecasting
   - Comparative cost analysis

3. **Integration Improvements**
   - Slack notifications for updates
   - Email reports for stakeholders
   - Integration with project management tools

4. **Data Validation**
   - Automated cost validation
   - Anomaly detection
   - Quality assurance checks

### Extension Points

#### Adding New Metrics
1. Update `scripts/update-cost-analysis.sh`
2. Add new calculation functions
3. Update documentation templates
4. Test with sample data

#### Custom Triggers
1. Modify GitHub Actions workflow
2. Add new trigger conditions
3. Update webhook handlers
4. Test trigger mechanisms

## 📚 Related Documentation

- [Cost Assumptions Document](../business/COST_ASSUMPTIONS_AND_PRODUCTIVITY_ANALYSIS.md)
- [Data Directory README](../../data/README.md)
- [Main Project README](../../README.md)
- [GitHub Actions Documentation](https://docs.github.com/en/actions)

## 🤝 Contributing

### Adding New Cost Metrics
1. Update the calculation functions in `update-cost-analysis.sh`
2. Add new fields to the JSON schema
3. Update documentation templates
4. Test with sample data
5. Update this documentation

### Modifying Update Logic
1. Edit the relevant functions in the update script
2. Test changes with sample data
3. Update error handling if needed
4. Document changes in this file
5. Test the full workflow

---

*This documentation is automatically maintained as part of the EconGraph cost analysis system.*
*Last Updated: September 2025*
*Version: 1.0*
