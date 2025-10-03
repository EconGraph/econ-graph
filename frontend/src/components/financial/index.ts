// Financial UI Components Export Index
// This file exports all financial-related UI components for easy importing

export { FinancialDashboard } from './FinancialDashboard';
export { FinancialStatementViewer } from './FinancialStatementViewer';
export { RatioAnalysisPanel } from './RatioAnalysisPanel';
export { BenchmarkComparison } from './BenchmarkComparison';
export { AnnotationPanel } from './AnnotationPanel';
export { EducationalPanel } from './EducationalPanel';
export { CollaborativePresence } from './CollaborativePresence';
export { RatioExplanationModal } from './RatioExplanationModal';
export { TrendAnalysisChart } from './TrendAnalysisChart';
export { PeerComparisonChart } from './PeerComparisonChart';
export { FinancialAlerts } from './FinancialAlerts';
export { FinancialExport } from './FinancialExport';
export { FinancialMobile } from './FinancialMobile';

// Re-export types for convenience
export type {
  FinancialStatement,
  FinancialLineItem,
  FinancialRatio,
  Company,
  FinancialAnnotation,
  AnnotationReply,
  AnnotationAssignment,
  AnnotationTemplate,
} from '@/types/financial';

// Re-export GraphQL queries and mutations
export {
  GET_FINANCIAL_STATEMENTS,
  GET_FINANCIAL_LINE_ITEMS,
  GET_FINANCIAL_RATIOS,
  GET_COMPANY_INFO,
  GET_FINANCIAL_ANNOTATIONS,
  CREATE_FINANCIAL_ANNOTATION,
  UPDATE_FINANCIAL_ANNOTATION,
  DELETE_FINANCIAL_ANNOTATION,
  FINANCIAL_ANNOTATION_SUBSCRIPTION,
} from '@/graphql/financial';
