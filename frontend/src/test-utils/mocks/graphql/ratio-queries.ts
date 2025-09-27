// GraphQL queries for ratio analysis components
// These queries are used for MSW mocking and schema validation

export const GET_FINANCIAL_RATIOS = `
  query GetFinancialRatios($statementId: ID!) {
    financialRatios(statementId: $statementId) {
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
  }
`;

export const GET_RATIO_BENCHMARKS = `
  query GetRatioBenchmarks($ratioName: String!, $industry: String) {
    ratioBenchmarks(ratioName: $ratioName, industry: $industry) {
      ratioName
      industry
      percentile25
      percentile50
      percentile75
      percentile90
      sampleSize
      lastUpdated
    }
  }
`;

export const GET_RATIO_EXPLANATION = `
  query GetRatioExplanation($ratioName: String!) {
    ratioExplanation(ratioName: $ratioName) {
      ratioName
      displayName
      formula
      description
      interpretation
      category
      importance
      warrenBuffettFavorite
      analystPreferred
      calculationMethod
      limitations
      relatedRatios
    }
  }
`;
