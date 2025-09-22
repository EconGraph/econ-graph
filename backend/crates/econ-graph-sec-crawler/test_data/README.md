# XBRL Test Data

This directory contains sample XBRL files for testing the SEC EDGAR crawler and XBRL parser.

## Files

### `sample_10k.xml` (6.3K)
- **Type**: Synthetic XBRL instance document
- **Company**: Apple Inc. (CIK: 0000320193)
- **Period**: 2023 Annual and Q4 2023
- **Purpose**: Unit testing with known, controlled data
- **Content**: 
  - Balance sheet items (Assets, Liabilities, Equity)
  - Income statement items (Revenue, Net Income, etc.)
  - Cash flow statement items
  - Proper XBRL contexts, units, and facts

### `apple_2025_q3_10q.xml` (760K)
- **Type**: Real SEC EDGAR XBRL filing
- **Company**: Apple Inc. (CIK: 0000320193)
- **Filing**: 10-Q for Q3 2025 (period ending June 28, 2025)
- **Accession Number**: 000032019325000073
- **Purpose**: Integration testing with real-world data
- **Content**: Complete Apple quarterly filing with:
  - Full financial statements
  - Complex XBRL taxonomy references
  - Multiple contexts and dimensions
  - Real financial data and calculations

### `jpmorgan_2025_q2_10q.xml` (13MB)
- **Type**: Real SEC EDGAR XBRL filing
- **Company**: JPMorgan Chase & Co. (CIK: 0000019617)
- **Filing**: 10-Q for Q2 2025 (period ending June 30, 2025)
- **Accession Number**: 0000019617-25-000615
- **Purpose**: Testing bank-specific XBRL taxonomies and financial concepts
- **Content**: Complete JPMorgan quarterly filing with:
  - Bank-specific financial statements
  - Loan portfolios and credit facilities
  - Deposit and liability structures
  - Regulatory capital requirements
  - Interest rate and credit risk disclosures

### `chevron_2025_q2_10q.xml` (1.8MB)
- **Type**: Real SEC EDGAR XBRL filing
- **Company**: Chevron Corporation (CIK: 0000093410)
- **Filing**: 10-Q for Q2 2025 (period ending June 30, 2025)
- **Accession Number**: 0000093410-25-000062
- **Purpose**: Testing oil/gas industry-specific XBRL taxonomies
- **Content**: Complete Chevron quarterly filing with:
  - Oil and gas production metrics
  - Exploration and development costs
  - Proven and probable reserves
  - Refining and marketing operations
  - Environmental and regulatory disclosures

### `apple_2023_q4.xsd` (325B)
- **Type**: XBRL taxonomy schema (error response)
- **Purpose**: Testing error handling for missing files

### `apple_2023_q4_htm.xml` (329B)
- **Type**: Error response from SEC
- **Purpose**: Testing error handling for invalid URLs

## Usage in Tests

These files are used by the XBRL parser tests to verify:

1. **Basic XBRL parsing** - `sample_10k.xml`
2. **Technology company complexity** - `apple_2025_q3_10q.xml`
3. **Banking industry taxonomies** - `jpmorgan_2025_q2_10q.xml`
4. **Oil/gas industry taxonomies** - `chevron_2025_q2_10q.xml`
5. **Error handling** - Invalid files
6. **Performance testing** - Large real files (13MB+)
7. **Data extraction** - Financial statement mapping
8. **Context resolution** - Multiple periods and entities
9. **Industry-specific concepts** - Loans, deposits, reserves, production

## Data Sources

- **Real filings**: Downloaded from SEC EDGAR database
- **Synthetic data**: Created for controlled testing
- **User-Agent**: EconGraph Financial Analysis Tool (contact@example.com)

## Legal Notice

Real XBRL filings are public domain data from the SEC EDGAR database. They are used here for testing purposes only and remain the property of the respective companies and the SEC.
