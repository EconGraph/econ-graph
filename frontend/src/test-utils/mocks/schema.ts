// GraphQL schema for MSW mocking and validation
// This schema defines the structure of our GraphQL API for testing

import { buildSchema } from 'graphql';
import { addMocksToSchema } from '@graphql-tools/mock';

const schema = buildSchema(`
  type Query {
    series(id: ID!): Series
    seriesData(seriesId: ID!, filter: SeriesDataFilter, transformation: DataTransformation): SeriesDataConnection
    searchSeries(params: SearchParams!): SearchResults
    searchSuggestions(partialQuery: String!, limit: Int): [SearchSuggestion]
    dataSources: [DataSource]
    crawlerStatus: CrawlerStatus
    company(id: ID!): Company
    financialStatement(id: ID!): FinancialStatement
    chartData(seriesId: ID!, timeRange: String!): ChartData
    currentUser: User
  }

  type Mutation {
    login(email: String!, password: String!): LoginResult
    logout: Boolean
  }

  type Series {
    id: ID!
    title: String!
    description: String
    sourceId: String
    frequency: String
    units: String
    lastUpdated: String
    startDate: String
    endDate: String
    similarityScore: Float
  }

  type SeriesDataConnection {
    nodes: [DataPoint]
    totalCount: Int
    pageInfo: PageInfo
  }

  type DataPoint {
    date: String!
    value: Float!
    revisionDate: String
    isOriginalRelease: Boolean
  }

  type PageInfo {
    hasNextPage: Boolean!
    hasPreviousPage: Boolean!
    startCursor: String
    endCursor: String
  }

  type SearchResults {
    results: [Series]
    totalCount: Int
  }

  type SearchSuggestion {
    suggestion: String!
    type: String
  }

  type DataSource {
    id: ID!
    name: String!
    description: String
    baseUrl: String
    apiKeyRequired: Boolean
    rateLimitPerMinute: Int
    seriesCount: Int
    createdAt: String
    updatedAt: String
  }

  type CrawlerStatus {
    isRunning: Boolean!
    lastRunAt: String
    nextRunAt: String
    totalJobs: Int
    completedJobs: Int
    failedJobs: Int
    pendingJobs: Int
  }

  type QueueStatistics {
    totalItems: Int
    pendingItems: Int
    processingItems: Int
    completedItems: Int
    failedItems: Int
  }

  type Company {
    id: ID!
    name: String!
    ticker: String
    industry: String
    sector: String
    financialStatements: [FinancialStatement]
  }

  type FinancialStatement {
    id: ID!
    type: FinancialStatementType!
    period: String!
    data: JSON
    lineItems: [LineItem]
  }

  type LineItem {
    id: ID!
    name: String!
    value: Float
    category: String
  }

  type ChartData {
    seriesId: ID!
    timeRange: String!
    dataPoints: [DataPoint]
    metadata: ChartMetadata
  }

  type ChartMetadata {
    title: String!
    units: String
    frequency: String
  }

  type User {
    id: ID!
    name: String!
    email: String!
    role: UserRole!
  }

  type LoginResult {
    user: User
    token: String
  }

  enum FinancialStatementType {
    INCOME_STATEMENT
    BALANCE_SHEET
    CASH_FLOW_STATEMENT
  }

  enum UserRole {
    USER
    ADMIN
    EXPERT
  }

  input SeriesDataFilter {
    startDate: String
    endDate: String
    limit: Int
  }

  input DataTransformation {
    type: String
    parameters: JSON
  }

  input SearchParams {
    query: String!
    limit: Int
    sourceIds: [String]
  }

  scalar JSON
`);

export const mockedSchema = addMocksToSchema({ schema });
export { schema };
