/**
 * REQUIREMENT: GraphQL integration for efficient data fetching
 * PURPOSE: Provide GraphQL client configuration and query utilities
 * This enables efficient data fetching with the Rust backend GraphQL API
 */

const GRAPHQL_ENDPOINT = process.env.REACT_APP_GRAPHQL_URL || '/graphql';

export interface GraphQLResponse<T = any> {
  data?: T;
  errors?: Array<{
    message: string;
    locations?: Array<{ line: number; column: number }>;
    path?: string[];
  }>;
}

export interface GraphQLRequest {
  query: string;
  variables?: Record<string, any>;
  operationName?: string;
}

/**
 * Execute a GraphQL query against the backend
 */
export async function executeGraphQL<T = any>(
  request: GraphQLRequest
): Promise<GraphQLResponse<T>> {
  const response = await fetch(GRAPHQL_ENDPOINT, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Accept: 'application/json',
    },
    body: JSON.stringify(request),
  });

  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }

  const result: GraphQLResponse<T> = await response.json();

  if (result.errors && result.errors.length > 0) {
    console.error('GraphQL errors:', result.errors);
    throw new Error(result.errors[0].message);
  }

  return result;
}

// Common GraphQL queries
export const QUERIES = {
  // Get economic series list with filtering
  GET_SERIES_LIST: `
    query GetSeriesList(
      $filter: SeriesFilter
      $pagination: PaginationInput
    ) {
      seriesList(filter: $filter, pagination: $pagination) {
        nodes {
          id
          title
          description
          source {
            name
          }
          frequency
          units
          startDate
          endDate
          lastUpdated
          dataPointCount
        }
        totalCount
        pageInfo {
          hasNextPage
          hasPreviousPage
          startCursor
          endCursor
        }
      }
    }
  `,

  // Get detailed series information
  GET_SERIES_DETAIL: `
    query GetSeriesDetail($id: ID!) {
      series(id: $id) {
        id
        title
        description
        source {
          name
          description
        }
        frequency
        units
        seasonalAdjustment
        startDate
        endDate
        lastUpdated
        isActive
        dataPointCount
      }
    }
  `,

  // Get series data with transformations
  GET_SERIES_DATA: `
    query GetSeriesData(
      $seriesId: ID!
      $filter: DataFilter
      $transformation: DataTransformation
      $first: Int
      $after: String
    ) {
      seriesData(
        seriesId: $seriesId
        filter: $filter
        transformation: $transformation
        first: $first
        after: $after
      ) {
        nodes {
          date
          value
          revisionDate
          isOriginalRelease
        }
        totalCount
        pageInfo {
          hasNextPage
          hasPreviousPage
          startCursor
          endCursor
        }
      }
    }
  `,

  // Full-text search with spelling correction and synonyms
  SEARCH_SERIES_FULLTEXT: `
    query SearchSeriesFulltext($params: SearchParamsInput!) {
      searchSeries(params: $params) {
        id
        title
        description
        externalId
        sourceId
        frequency
        units
        startDate
        endDate
        lastUpdated
        isActive
        rank
        similarityScore
      }
    }
  `,

  // Get search suggestions for query completion and spelling correction
  GET_SEARCH_SUGGESTIONS: `
    query GetSearchSuggestions($partialQuery: String!, $limit: Int) {
      searchSuggestions(partialQuery: $partialQuery, limit: $limit) {
        suggestion
        matchCount
        suggestionType
        confidence
      }
    }
  `,

  // Legacy search series (kept for backward compatibility)
  SEARCH_SERIES: `
    query SearchSeries(
      $query: String!
      $source: String
      $frequency: SeriesFrequencyType
      $first: Int
      $after: String
    ) {
      searchSeries(
        query: $query
        source: $source
        frequency: $frequency
        first: $first
        after: $after
      ) {
        series {
          id
          title
          description
          source {
            name
          }
          frequency
          units
          lastUpdated
        }
        totalCount
        query
        tookMs
      }
    }
  `,

  // Get data sources
  GET_DATA_SOURCES: `
    query GetDataSources {
      dataSources {
        id
        name
        description
        baseUrl
        apiKeyRequired
        rateLimitPerMinute
        seriesCount
        createdAt
        updatedAt
      }
    }
  `,

  // Get crawler status for monitoring
  GET_CRAWLER_STATUS: `
    query GetCrawlerStatus {
      crawlerStatus {
        isRunning
        activeWorkers
        lastCrawl
        nextScheduledCrawl
      }
      queueStatistics {
        totalItems
        pendingItems
        processingItems
        completedItems
        failedItems
        retryingItems
        oldestPending
        averageProcessingTime
      }
    }
  `,

  // Collaboration queries
  GET_ANNOTATIONS_FOR_SERIES: `
    query GetAnnotationsForSeries($seriesId: String!, $userId: ID) {
      annotationsForSeries(seriesId: $seriesId, userId: $userId) {
        id
        userId
        seriesId
        chartId
        annotationDate
        annotationValue
        title
        description
        color
        annotationType
        isVisible
        isPinned
        tags
        createdAt
        updatedAt
      }
    }
  `,

  GET_COMMENTS_FOR_ANNOTATION: `
    query GetCommentsForAnnotation($annotationId: ID!) {
      commentsForAnnotation(annotationId: $annotationId) {
        id
        annotationId
        userId
        content
        isResolved
        createdAt
        updatedAt
      }
    }
  `,

  GET_CHART_COLLABORATORS: `
    query GetChartCollaborators($chartId: ID!) {
      chartCollaborators(chartId: $chartId) {
        id
        chartId
        userId
        invitedBy
        role
        permissions
        createdAt
        lastAccessedAt
      }
    }
  `,

  GET_USER: `
    query GetUser($userId: ID!) {
      user(userId: $userId) {
        id
        email
        name
        avatarUrl
        provider
        role
        organization
        theme
        defaultChartType
        notificationsEnabled
        collaborationEnabled
        isActive
        emailVerified
        createdAt
        updatedAt
        lastLoginAt
      }
    }
  `,

  // Global Analysis queries
  GET_COUNTRIES_WITH_ECONOMIC_DATA: `
    query GetCountriesWithEconomicData {
      countriesWithEconomicData {
        id
        name
        isoAlpha2
        isoAlpha3
        region
        subRegion
        latitude
        longitude
        gdpUsd
        population
        economicIndicators {
          indicatorName
          indicatorCode
          value
          date
        }
      }
    }
  `,

  GET_CORRELATION_NETWORK: `
    query GetCorrelationNetwork($indicatorCategory: String) {
      correlationNetwork(indicatorCategory: $indicatorCategory) {
        countryAId
        countryBId
        indicatorCode
        correlationCoefficient
        pValue
        startDate
        endDate
        countryA {
          name
          isoAlpha2
        }
        countryB {
          name
          isoAlpha2
        }
      }
    }
  `,

  GET_GLOBAL_EVENTS_WITH_IMPACTS: `
    query GetGlobalEventsWithImpacts($minImpactScore: Int) {
      globalEventsWithImpacts(minImpactScore: $minImpactScore) {
        id
        name
        description
        eventType
        severity
        startDate
        endDate
        countryImpacts {
          country {
            name
            isoAlpha2
          }
          impactSeverity
          recoveryStatus
          impactDescription
        }
      }
    }
  `,
};

// Common GraphQL mutations
export const MUTATIONS = {
  // Trigger manual crawl
  TRIGGER_CRAWL: `
    mutation TriggerCrawl($input: TriggerCrawlInput!) {
      triggerCrawl(input: $input) {
        isRunning
        activeWorkers
        lastCrawl
        nextScheduledCrawl
      }
    }
  `,

  // Collaboration mutations
  CREATE_ANNOTATION: `
    mutation CreateAnnotation($input: CreateAnnotationInput!) {
      createAnnotation(input: $input) {
        id
        userId
        seriesId
        chartId
        annotationDate
        annotationValue
        title
        description
        color
        annotationType
        isVisible
        isPinned
        tags
        createdAt
        updatedAt
      }
    }
  `,

  ADD_COMMENT: `
    mutation AddComment($input: AddCommentInput!) {
      addComment(input: $input) {
        id
        annotationId
        userId
        content
        isResolved
        createdAt
        updatedAt
      }
    }
  `,

  SHARE_CHART: `
    mutation ShareChart($input: ShareChartInput!) {
      shareChart(input: $input) {
        id
        chartId
        userId
        invitedBy
        role
        permissions
        createdAt
        lastAccessedAt
      }
    }
  `,

  DELETE_ANNOTATION: `
    mutation DeleteAnnotation($input: DeleteAnnotationInput!) {
      deleteAnnotation(input: $input)
    }
  `,
};

// Type definitions for better TypeScript support
export interface SeriesListNode {
  id: string;
  title: string;
  description: string;
  source: {
    name: string;
  };
  frequency: string;
  units: string;
  startDate: string;
  endDate: string;
  lastUpdated: string;
  dataPointCount: number;
}

export interface SeriesListResponse {
  seriesList: {
    nodes: SeriesListNode[];
    totalCount: number;
    pageInfo: {
      hasNextPage: boolean;
      hasPreviousPage: boolean;
      startCursor?: string;
      endCursor?: string;
    };
  };
}

export interface SeriesDetailResponse {
  series: {
    id: string;
    title: string;
    description: string;
    source: {
      name: string;
      description: string;
    };
    frequency: string;
    units: string;
    seasonalAdjustment?: string;
    startDate: string;
    endDate: string;
    lastUpdated: string;
    isActive: boolean;
    dataPointCount: number;
  };
}

export interface DataPoint {
  date: string;
  value: number | null;
  revisionDate: string;
  isOriginalRelease: boolean;
}

export interface SeriesDataResponse {
  seriesData: {
    nodes: DataPoint[];
    totalCount: number;
    pageInfo: {
      hasNextPage: boolean;
      hasPreviousPage: boolean;
      startCursor?: string;
      endCursor?: string;
    };
  };
}

export interface SearchSeriesResponse {
  searchSeries: {
    series: SeriesListNode[];
    totalCount: number;
    query: string;
    tookMs: number;
  };
}

// Collaboration types
export interface ChartAnnotationType {
  id: string;
  userId: string;
  seriesId?: string;
  chartId?: string;
  annotationDate: string;
  annotationValue?: number;
  title: string;
  description?: string;
  color?: string;
  annotationType?: string;
  isVisible?: boolean;
  isPinned?: boolean;
  tags?: string[];
  createdAt?: string;
  updatedAt?: string;
}

export interface AnnotationCommentType {
  id: string;
  annotationId: string;
  userId: string;
  content: string;
  isResolved?: boolean;
  createdAt?: string;
  updatedAt?: string;
}

export interface ChartCollaboratorType {
  id: string;
  chartId: string;
  userId: string;
  invitedBy?: string;
  role?: string;
  permissions?: string;
  createdAt?: string;
  lastAccessedAt?: string;
}

export interface UserType {
  id: string;
  email: string;
  name: string;
  avatarUrl?: string;
  provider: string;
  role: string;
  organization?: string;
  theme?: string;
  defaultChartType?: string;
  notificationsEnabled?: boolean;
  collaborationEnabled?: boolean;
  isActive?: boolean;
  emailVerified?: boolean;
  createdAt?: string;
  updatedAt?: string;
  lastLoginAt?: string;
}

// Collaboration input types
export interface CreateAnnotationInput {
  userId: string;
  seriesId: string;
  annotationDate: string;
  annotationValue?: number;
  title: string;
  content: string;
  annotationType: string;
  color?: string;
  isPublic?: boolean;
}

export interface AddCommentInput {
  userId: string;
  annotationId: string;
  content: string;
}

export interface ShareChartInput {
  ownerUserId: string;
  targetUserId: string;
  chartId: string;
  permissionLevel: string;
}

export interface DeleteAnnotationInput {
  userId: string;
  annotationId: string;
}

// Response types
export interface AnnotationsForSeriesResponse {
  annotationsForSeries: ChartAnnotationType[];
}

export interface CommentsForAnnotationResponse {
  commentsForAnnotation: AnnotationCommentType[];
}

export interface ChartCollaboratorsResponse {
  chartCollaborators: ChartCollaboratorType[];
}

export interface UserResponse {
  user: UserType | null;
}
