import { gql } from '@apollo/client';

export const GET_FINANCIAL_STATEMENTS = gql`
  query GetFinancialStatements($companyId: String!, $limit: Int, $offset: Int) {
    financialStatements(companyId: $companyId, limit: $limit, offset: $offset) {
      id
      companyId
      filingType
      formType
      accessionNumber
      filingDate
      periodEndDate
      fiscalYear
      fiscalQuarter
      documentType
      documentUrl
      xbrlProcessingStatus
      xbrlProcessingError
      xbrlProcessingStartedAt
      xbrlProcessingCompletedAt
      isAmended
      amendmentType
      originalFilingDate
      isRestated
      restatementReason
      createdAt
      updatedAt
    }
  }
`;

export const GET_FINANCIAL_LINE_ITEMS = gql`
  query GetFinancialLineItems($statementId: String!) {
    financialLineItems(statementId: $statementId) {
      id
      statementId
      taxonomyConcept
      standardLabel
      value
      unit
      contextRef
      statementType
      statementSection
      isCalculated
      calculationWeight
      parentConcept
      createdAt
      updatedAt
    }
  }
`;

export const GET_FINANCIAL_RATIOS = gql`
  query GetFinancialRatios($statementId: String!) {
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

export const GET_COMPANY_INFO = gql`
  query GetCompanyInfo($cik: String!) {
    company(cik: $cik) {
      id
      cik
      name
      ticker
      sic
      sicDescription
      gics
      gicsDescription
      businessAddress
      mailingAddress
      phone
      website
      stateOfIncorporation
      stateOfIncorporationDescription
      fiscalYearEnd
      businessStatus
      createdAt
      updatedAt
    }
  }
`;

export const GET_FINANCIAL_ANNOTATIONS = gql`
  query GetFinancialAnnotations($statementId: String!) {
    financialAnnotations(statementId: $statementId) {
      id
      statementId
      lineItemId
      authorId
      content
      annotationType
      tags
      highlights
      mentions
      parentAnnotationId
      status
      isPrivate
      createdAt
      updatedAt
    }
  }
`;

export const CREATE_FINANCIAL_ANNOTATION = gql`
  mutation CreateFinancialAnnotation($input: CreateFinancialAnnotationInput!) {
    createFinancialAnnotation(input: $input) {
      id
      statementId
      lineItemId
      authorId
      content
      annotationType
      tags
      highlights
      mentions
      parentAnnotationId
      status
      isPrivate
      createdAt
      updatedAt
    }
  }
`;

export const UPDATE_FINANCIAL_ANNOTATION = gql`
  mutation UpdateFinancialAnnotation($id: String!, $input: UpdateFinancialAnnotationInput!) {
    updateFinancialAnnotation(id: $id, input: $input) {
      id
      statementId
      lineItemId
      authorId
      content
      annotationType
      tags
      highlights
      mentions
      parentAnnotationId
      status
      isPrivate
      createdAt
      updatedAt
    }
  }
`;

export const DELETE_FINANCIAL_ANNOTATION = gql`
  mutation DeleteFinancialAnnotation($id: String!) {
    deleteFinancialAnnotation(id: $id)
  }
`;

export const FINANCIAL_ANNOTATION_SUBSCRIPTION = gql`
  subscription FinancialAnnotationSubscription($statementId: String!) {
    financialAnnotationUpdated(statementId: $statementId) {
      id
      statementId
      lineItemId
      authorId
      content
      annotationType
      tags
      highlights
      mentions
      parentAnnotationId
      status
      isPrivate
      createdAt
      updatedAt
    }
  }
`;
