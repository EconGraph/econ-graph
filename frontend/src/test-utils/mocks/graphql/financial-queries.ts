// GraphQL queries for financial components
// These queries are used for MSW mocking and schema validation

export const GET_FINANCIAL_DASHBOARD = `
  query GetFinancialDashboard($companyId: ID!) {
    company(id: $companyId) {
      id
      name
      ticker
      industry
      sector
      financialStatements {
        id
        type
        period
        data
      }
      financialRatios {
        id
        statementId
        ratioName
        ratioDisplayName
        value
        category
        formula
        interpretation
        benchmarkPercentile
        periodEndDate
        fiscalYear
        fiscalQuarter
        calculatedAt
        dataQualityScore
      }
      trends {
        id
        ratioName
        period
        value
        change
        direction
      }
    }
  }
`;

export const GET_FINANCIAL_STATEMENT = `
  query GetFinancialStatement($statementId: ID!) {
    financialStatement(id: $statementId) {
      id
      type
      period
      data
      lineItems {
        id
        name
        value
        category
      }
    }
  }
`;

export const GET_TREND_ANALYSIS = `
  query GetTrendAnalysis($companyId: ID!, $timeRange: String!) {
    company(id: $companyId) {
      id
      name
      financialStatements {
        id
        type
        period
        lineItems {
          id
          name
          value
          category
        }
      }
    }
  }
`;

export const GET_PEER_COMPARISON = `
  query GetPeerComparison($companyId: ID!, $peerIds: [ID!]!) {
    company(id: $companyId) {
      id
      name
      ticker
      industry
      sector
    }
    peers: companies(ids: $peerIds) {
      id
      name
      ticker
      industry
      sector
    }
  }
`;

export const GET_PEER_COMPANIES = `
  query GetPeerCompanies($companyId: ID!) {
    peerCompanies(companyId: $companyId) {
      id
      name
      ticker
      industry
      marketCap
      ratios {
        name
        value
        percentile
      }
    }
  }
`;

export const GET_BENCHMARK_COMPARISON = `
  query GetBenchmarkComparison($companyId: ID!, $benchmarkType: String!) {
    company(id: $companyId) {
      id
      name
      ticker
      industry
      sector
    }
    benchmark(type: $benchmarkType) {
      id
      name
      type
      value
    }
  }
`;

export const GET_FINANCIAL_ALERTS = `
  query GetFinancialAlerts($companyId: ID!) {
    company(id: $companyId) {
      id
      name
      alerts {
        id
        type
        severity
        message
        createdAt
        resolved
      }
    }
  }
`;

export const GET_FINANCIAL_RATIOS = `
  query GetFinancialRatios($statementId: ID!) {
    financialStatement(id: $statementId) {
      id
      financialRatios {
        id
        name
        value
        category
        benchmark
      }
    }
  }
`;

export const GET_RATIO_BENCHMARKS = `
  query GetRatioBenchmarks($statementId: ID!) {
    financialStatement(id: $statementId) {
      id
      ratioBenchmarks {
        id
        name
        value
        benchmarkText
        performanceLevel
      }
    }
  }
`;

export const GET_FINANCIAL_EXPORT = `
  query GetFinancialExport($companyId: ID!, $format: String!) {
    company(id: $companyId) {
      id
      name
      ticker
      financialStatements {
        id
        type
        period
        data
      }
    }
  }
`;

export const GET_FINANCIAL_ANNOTATIONS = `
  query GetFinancialAnnotations($statementId: ID!) {
    annotations(statementId: $statementId) {
      id
      content
      type
      lineItemId
      createdAt
      updatedAt
    }
  }
`;
