# Cost Assumptions and Productivity Analysis

> **Product Manager Documentation**: Comprehensive analysis of software development costs and productivity metrics with cited sources for EconGraph project cost assumptions.

## Executive Summary

This document provides detailed cost assumptions and productivity analysis for the EconGraph project, with specific citations to industry sources and academic research. All cost estimates are based on current market data and industry benchmarks.

**📊 Live Cost Data**: This document is automatically updated from source data. See [data/cost-analysis.json](../../data/cost-analysis.json) for the latest figures.

**🔄 Auto-Update**: Run `./scripts/update-cost-analysis.sh` to update all cost figures from source data.

## Software Development Cost Assumptions

### 1. Developer Hourly Rates by Region and Experience Level

#### United States (Primary Market)
- **Junior Developers**: $35–$55 per hour
- **Mid-Level Developers**: $50–$90 per hour  
- **Senior Developers**: $78–$125 per hour

*Source: [Zymr Software Development Cost Analysis](https://www.zymr.com/blog/software-development-cost)*

#### Regional Variations
- **North America**: $100–$400 per hour
- **Western Europe**: $70–$150 per hour
- **Eastern Europe**: $20–$70 per hour
- **Latin America**: $30–$65 per hour
- **Asia-Pacific**: $20–$50 per hour

*Source: [LitsLink Offshore Development Rates](https://litslink.com/blog/cost-of-outsourcing-software-development)*

#### Company Size Impact
- **Big Business-Class Firms**: $250–$350 per hour
- **Mid-Market Firms**: $120–$250 per hour
- **Small-Class Firms**: $90–$160 per hour
- **Freelance Developers**: $50–$300 per hour

*Source: [FullStack Labs 2025 Software Development Price Guide](https://www.fullstack.com/labs/resources/blog/software-development-price-guide-hourly-rate-comparison)*

### 2. Total Employment Costs

Beyond hourly rates, total employment costs include:

- **Base Salary**: $100,000–$180,000 for experienced developers
- **Benefits and Payroll Taxes**: 30–40% of base salary
- **Office Space**: $25.43 per square foot nationally
- **Equipment and Tools**: $3,000–$5,000 per developer
- **Recruitment Costs**: $15,000–$25,000 per successful hire

*Source: [The Lean Product Studio Cost Guide](https://theleanproduct.studio/blog/software-development-cost-guide-2025-part-1)*

### 3. Project Complexity Cost Ranges

#### Simple Applications
- **Cost Range**: $10,000–$50,000
- **Timeline**: 1–3 months
- **Team Size**: 1–2 developers
- **Examples**: Basic CRUD apps, simple websites

#### Medium Complexity Applications
- **Cost Range**: $50,000–$200,000
- **Timeline**: 3–6 months
- **Team Size**: 2–4 developers
- **Examples**: E-commerce platforms, SaaS applications

#### Complex Enterprise Applications
- **Cost Range**: $200,000–$1,000,000+
- **Timeline**: 6–18 months
- **Team Size**: 4–10+ developers
- **Examples**: Financial systems, healthcare platforms, large-scale SaaS

*Source: [Clutch Software Development Cost Survey 2024](https://clutch.co/developers/software-development-cost)*

## EconGraph Project Cost Analysis

<!-- cost-analysis: generated; run scripts/update-cost-analysis.sh -->
All monetary estimates below are in USD. Line counts are physical newline counts
in the existing Git-tracked categories, not a measure of delivered functionality.

### Codebase Composition and Traditional Cost

**Total Codebase**: 207,286 lines of manually written code (selected Git-tracked categories; excludes lock files and generated cost JSON)

| Code Type | Lines | Share | Rate/Line | Base Cost |
|-----------|-------|-------|-----------|-----------|
| Production Code | 89,802 | 43.3% | $2.50 | $224,505.00 |
| Test Code | 17,713 | 8.5% | $1.25 | $22,141.25 |
| Infrastructure (configuration + scripts) | 46,959 | 22.7% | $1.00 | $46,959.00 |
| Documentation | 52,812 | 25.5% | $0.75 | $39,609.00 |
| **Total Base Cost** | **207,286** | **100.0%** | | **$333,214.25** |

- **Backend Production**: 53,650 lines
- **Backend Tests**: 11,981 lines
- **Frontend Production**: 36,152 lines
- **Frontend Tests**: 5,732 lines
- **Configuration Files**: 28,857 lines
- **Scripts and Automation**: 18,102 lines

| Cost Category | Amount |
|---------------|--------|
| Base Development Cost | $333,214.25 |
| Overhead (75% of base) | $249,910.69 |
| **Total Traditional Cost** | **$583,124.94** |

Overhead preserves the original 20% project management + 15% code reviews +
25% testing/QA + 15% integration/deployment assumptions. Monetary components
are rounded to cents with round-half-up; total cost is the sum of those components.
Shares are rounded independently and may not sum to exactly 100.0%.

### AI-Assisted Development Cost

| Component | Assumption | Amount |
|-----------|------------|--------|
| Staff engineer | 224 hours × $150/hour | $33,600.00 |
| Reported token usage | Sum of Cost column, rounded once | $1,288.55 |
| Cursor Pro | One month | $20.00 |
| **Total AI Tool Costs** | | **$1,308.55** |
| **Total AI-Assisted Cost** | | **$34,908.55** |

- **Total AI Interactions**: 6,768 CSV records
- **Total Tokens Processed**: 4,285,966,700 tokens (including cache reads and unsuccessful requests)
- **Reported CSV Cost**: 1288.554 USD before currency rounding
- **Daily Average**: $1,246.73 over the assumed 28 days

### Cost Comparison

| Development Approach | Total Cost | Cost per Counted Line |
|----------------------|------------|-----------------------|
| Traditional Development | $583,124.94 | $2.81 |
| AI-Assisted Development | $34,908.55 | $0.17 |
| **Savings** | **$548,216.39** | **94.0% reduction** |

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

## Productivity Multipliers

### 1. AI-Assisted Development Benefits

- **10-20x Faster Iteration Cycles**: Rapid prototyping and testing
- **200+ Hours Saved**: vs traditional solo development
- **Enterprise-Quality Results**: Professional testing, documentation, security
- **Reduced Learning Curve**: AI assistance with new technologies

### 2. Traditional Development Challenges

- **Coordination Overhead**: Team communication and knowledge transfer
- **Context Switching**: Multiple developers working on different components
- **Integration Complexity**: Merging code from multiple developers
- **Knowledge Silos**: Specialized expertise in different team members

### 3. Cost-Benefit Analysis

#### When to Use AI-Assisted Development
- **Solo or Small Team Projects**: 1-2 developers
- **Rapid Prototyping**: Fast iteration requirements
- **Learning New Technologies**: AI assistance with unfamiliar frameworks
- **Cost-Sensitive Projects**: Budget constraints requiring efficiency

#### When to Use Traditional Development
- **Large Team Projects**: 5+ developers with clear specialization
- **Complex Enterprise Systems**: Extensive business logic and integration
- **Regulatory Requirements**: Compliance-heavy industries
- **Long-term Maintenance**: Established teams with domain expertise

#### Hybrid Approach Recommendations
- **Start with AI-Assisted**: Rapid prototyping and initial development
- **Transition to Traditional**: For complex features requiring team collaboration
- **Use AI for Documentation**: Automated technical writing and API docs
- **Fall back to traditional**: For complex features requiring deep domain expertise

## Conclusion

The data presented in this document provides a comprehensive foundation for cost assumptions in software development projects. The dramatic cost differences between traditional and AI-assisted development (94.0% cost reduction) demonstrate the transformative potential of AI tools in software development.

**Key Takeaways**:
1. **Traditional Development**: $583,124.94 under the line-based model
2. **AI-Assisted Development**: $34,908.55 under the fixed staffing/usage assumptions
3. **Productivity Gains**: 10-20x faster development cycles
4. **Quality Maintenance**: Professional-grade results with AI assistance

This analysis supports the cost assumptions used in EconGraph project documentation and provides credible sources for any inquiries regarding development costs and productivity metrics.

---

*Last Updated: September 2025*
*Document Version: 1.0*
*Prepared by: Product Manager*

**📊 Source Data**: [data/cost-analysis.json](../../data/cost-analysis.json) | **🔄 Update Script**: [scripts/update-cost-analysis.sh](../../scripts/update-cost-analysis.sh)