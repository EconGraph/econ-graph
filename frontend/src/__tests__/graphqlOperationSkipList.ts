/**
 * GraphQL operations that are known not to validate against the backend schema
 * (`backend/crates/econ-graph-graphql/schema.graphql`).
 *
 * `graphql-operations.test.ts` checks every other operation in `src/`. An entry
 * here is still checked, the other way round: the test fails if the operation
 * now validates, or no longer exists, so the entry has to be removed in the PR
 * that fixes or deletes the operation.
 *
 * Keys are `<path under src/>#<OperationName>`. Values say which release 1 area
 * owns the fix. Nothing new may be added here for code on a reachable route;
 * the "Reachable today" group at the end must be empty before v4.0.0 (train 1) ships.
 * That is checked by hand against the train 1 exit criteria in
 * `docs/roadmap/releases.md`; no automated test enforces it.
 */
export const GRAPHQL_OPERATION_SKIP_LIST: Readonly<Record<string, string>> = {
  // Financial statements UI: compiled out of release 1 (sec area). No SEC
  // financial GraphQL API exists yet; these mocks and queries were written
  // ahead of it.
  'graphql/financial.ts#GetFinancialStatements': 'sec: compiled out',
  'graphql/financial.ts#GetFinancialLineItems': 'sec: compiled out',
  'graphql/financial.ts#GetFinancialRatios': 'sec: compiled out',
  'graphql/financial.ts#GetCompanyInfo': 'sec: compiled out',
  'graphql/financial.ts#GetFinancialAnnotations': 'sec: compiled out',
  'graphql/financial.ts#CreateFinancialAnnotation': 'sec: compiled out',
  'graphql/financial.ts#UpdateFinancialAnnotation': 'sec: compiled out',
  'graphql/financial.ts#DeleteFinancialAnnotation': 'sec: compiled out',
  'graphql/financial.ts#FinancialAnnotationSubscription': 'sec: compiled out',
  'test-utils/mocks/graphql/financial-queries.ts#GetFinancialDashboard': 'sec: compiled out',
  'test-utils/mocks/graphql/financial-queries.ts#GetFinancialStatement': 'sec: compiled out',
  'test-utils/mocks/graphql/financial-queries.ts#GetTrendAnalysis': 'sec: compiled out',
  'test-utils/mocks/graphql/financial-queries.ts#GetPeerComparison': 'sec: compiled out',
  'test-utils/mocks/graphql/financial-queries.ts#GetPeerCompanies': 'sec: compiled out',
  'test-utils/mocks/graphql/financial-queries.ts#GetBenchmarkComparison': 'sec: compiled out',
  'test-utils/mocks/graphql/financial-queries.ts#GetFinancialAlerts': 'sec: compiled out',
  'test-utils/mocks/graphql/financial-queries.ts#GetFinancialRatios': 'sec: compiled out',
  'test-utils/mocks/graphql/financial-queries.ts#GetRatioBenchmarks': 'sec: compiled out',
  'test-utils/mocks/graphql/financial-queries.ts#GetFinancialExport': 'sec: compiled out',
  'test-utils/mocks/graphql/financial-queries.ts#GetFinancialAnnotations': 'sec: compiled out',
  'test-utils/mocks/graphql/ratio-queries.ts#GetFinancialRatios': 'sec: compiled out',
  'test-utils/mocks/graphql/ratio-queries.ts#GetRatioBenchmarks': 'sec: compiled out',
  'test-utils/mocks/graphql/ratio-queries.ts#GetRatioExplanation': 'sec: compiled out',

  // Global analysis tabs: compiled out (world-map area). The backend never
  // registered these fields (GlobalAnalysisQuery); train 1 replaces them with a
  // generic crossSection query. Nothing in src/ uses these three today.
  'utils/graphql.ts#GetCountriesWithEconomicData': 'world-map: replaced by crossSection',
  'utils/graphql.ts#GetCorrelationNetwork': 'world-map: replaced by crossSection',
  'utils/graphql.ts#GetGlobalEventsWithImpacts': 'world-map: replaced by crossSection',

  // Unused: nothing in src/ sends these. Their only caller, useCollaboration.ts,
  // was deleted by UI-8 (UI-5 deleted the other one, ChartCollaborationConnectedQuery,
  // and the now-uncalled GetAnnotations with it). Fix or delete them (series search
  // is #165).
  'utils/graphql.ts#GetChartCollaborators': 'series-ui: unused, UI-5 removed its other caller',
  'utils/graphql.ts#GetCommentsForAnnotation': 'series-ui: unused, UI-5 removed its other caller',
  'utils/graphql.ts#SearchSeriesFulltext': 'series-ui: unused, fix or delete with series search',

  // Reachable today. Must be gone before v4.0.0 (train 1).
  // The dashboard sends SearchSeries through useSeriesSearch.
  'utils/graphql.ts#SearchSeries': 'series-ui: reachable, UI-9 fixes it',
};
