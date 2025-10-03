/**
 * REQUIREMENT: GraphQL integration for efficient data fetching
 * PURPOSE: Provide GraphQL client configuration and query utilities
 * This enables efficient data fetching with the Rust backend GraphQL API.
 */

const GRAPHQL_ENDPOINT = import.meta.env.VITE_GRAPHQL_URL || '/graphql';

// Debug: Log the GraphQL endpoint being used
console.log('🔧 Frontend GraphQL Configuration:');
console.log('  - VITE_GRAPHQL_URL:', import.meta.env.VITE_GRAPHQL_URL);
console.log('  - Final GRAPHQL_ENDPOINT:', GRAPHQL_ENDPOINT);

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
 * Execute a GraphQL query against the backend.
 * @param request - The GraphQL request object containing query, variables, and operation name.
 * @returns Promise that resolves to the GraphQL response.
 */
export async function executeGraphQL<T = any>(
  request: GraphQLRequest
): Promise<GraphQLResponse<T>> {
  // Debug logging disabled for browser compatibility
  // if (MSW_DEBUG) {
  //   console.log('🔧 executeGraphQL called with:', { endpoint: GRAPHQL_ENDPOINT, request });
  // }
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
        user_id
        series_id
        chart_id
        annotation_date
        annotation_value
        title
        description
        color
        annotation_type
        is_visible
        is_pinned
        tags
        created_at
        updated_at
      }
    }
  `,

  GET_COMMENTS_FOR_ANNOTATION: `
    query GetCommentsForAnnotation($annotationId: ID!) {
      commentsForAnnotation(annotationId: $annotationId) {
        id
        annotation_id
        user_id
        content
        is_resolved
        created_at
        updated_at
      }
    }
  `,

  GET_CHART_COLLABORATORS: `
    query GetChartCollaborators($chartId: ID!) {
      chartCollaborators(chartId: $chartId) {
        id
        chart_id
        user_id
        invited_by
        role
        permissions
        created_at
        last_accessed_at
      }
    }
  `,

  GET_ANNOTATIONS: `
    query GetAnnotations($chartId: ID!) {
      annotationsForChart(chartId: $chartId) {
        id
        user_id
        chart_id
        title
        description
        content
        annotation_type
        is_visible
        is_pinned
        created_at
        updated_at
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
  user_id: string;
  series_id?: string;
  chart_id?: string;
  annotation_date: string;
  annotation_value?: number;
  title: string;
  description?: string;
  color?: string;
  annotation_type?: string;
  is_visible?: boolean;
  is_pinned?: boolean;
  tags?: string[];
  created_at?: string;
  updated_at?: string;
}

export interface AnnotationCommentType {
  id: string;
  annotation_id: string;
  user_id: string;
  content: string;
  is_resolved?: boolean;
  created_at?: string;
  updated_at?: string;
}

export interface ChartCollaboratorType {
  id: string;
  chart_id: string;
  user_id: string;
  invited_by?: string;
  role?: string;
  permissions?: string;
  created_at?: string;
  last_accessed_at?: string;
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
  user_id: string;
  series_id: string;
  annotation_date: string;
  annotation_value?: number;
  title: string;
  content: string;
  annotation_type: string;
  color?: string;
  is_public?: boolean;
}

export interface AddCommentInput {
  user_id: string;
  annotation_id: string;
  content: string;
}

export interface ShareChartInput {
  owner_user_id: string;
  target_user_id: string;
  chart_id: string;
  permission_level: string;
}

export interface DeleteAnnotationInput {
  user_id: string;
  annotation_id: string;
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
