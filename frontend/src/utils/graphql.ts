/**
 * REQUIREMENT: GraphQL integration for efficient data fetching
 * PURPOSE: Provide GraphQL client configuration and query utilities
 * This enables efficient data fetching with the Rust backend GraphQL API.
 */

import { getAccessToken, isAccountSuspended, setAccountSuspended } from '../auth/accessToken';

const GRAPHQL_ENDPOINT = import.meta.env.VITE_GRAPHQL_URL || '/graphql';

/**
 * Where data operations go. Unset, it is the normal GraphQL URL, so a build without
 * `VITE_DATA_GRAPHQL_URL` sends every request where it went before. Branch deployments set it
 * to the same-origin `/data/graphql`, which reads the shared primary data plane (ECO-427).
 */
const DATA_GRAPHQL_ENDPOINT = import.meta.env.VITE_DATA_GRAPHQL_URL || GRAPHQL_ENDPOINT;

// Debug: Log the GraphQL endpoints being used
console.log('🔧 Frontend GraphQL Configuration:');
console.log('  - VITE_GRAPHQL_URL:', import.meta.env.VITE_GRAPHQL_URL);
console.log('  - VITE_DATA_GRAPHQL_URL:', import.meta.env.VITE_DATA_GRAPHQL_URL);
console.log('  - Final GRAPHQL_ENDPOINT (app operations):', GRAPHQL_ENDPOINT);
console.log('  - Final DATA_GRAPHQL_ENDPOINT (data operations):', DATA_GRAPHQL_ENDPOINT);

/**
 * Data operations read data-plane tables and go to the data URL. Signed-in requests carry
 * the caller's token so the data backend can enforce catalog roles when it shares our trust.
 * A separate data URL stays anonymous unless VITE_DATA_GRAPHQL_AUTH=true, preserving access
 * to shared data planes that use a different deployment's Keycloak realm. The routing audit
 * behind this list is in ECO-427. `graphql-operations.test.ts` fails if an operation in `src/` is in
 * neither this set nor `APP_OPERATIONS`, or in both.
 */
export const DATA_OPERATIONS: ReadonlySet<string> = new Set([
  // Shipped in the release build.
  'GetSeriesDetail',
  'GetSeriesData',
  'SearchSeries',
  'GetSeriesList',
  'GetDataSources',
  'GetSeriesLatestObservation',
  'GetMapDatasets',
  'GetMapCrossSection',
  'GetMapSeriesMeta',
  // Defined but not sent by any page.
  'GetSearchSuggestions',
  'SearchSeriesFulltext',
  // Compiled out (`financial_components`); classified so turning the flag on routes them right.
  'GetFinancialStatements',
  'GetFinancialLineItems',
  'GetFinancialRatios',
  'GetCompanyInfo',
  'GetFinancialDashboard',
  'GetFinancialStatement',
  'GetTrendAnalysis',
  'GetPeerComparison',
  'GetPeerCompanies',
  'GetBenchmarkComparison',
  'GetFinancialAlerts',
  'GetRatioBenchmarks',
  'GetRatioExplanation',
  'GetFinancialExport',
]);

/**
 * App operations: they use the app database or the caller, so they go to the GraphQL URL with
 * the user's token. Unknown and unnamed operations are sent the same way.
 */
export const APP_OPERATIONS: ReadonlySet<string> = new Set([
  'GetAnnotationsForSeries',
  'GetCommentsForAnnotation',
  'CreateAnnotation',
  'UpdateAnnotation',
  'DeleteAnnotation',
  'AddComment',
  // Unused.
  'GetChartCollaborators',
  'GetUser',
  'ShareChart',
  // Compiled out (`financial_components`).
  'GetFinancialAnnotations',
  'CreateFinancialAnnotation',
  'UpdateFinancialAnnotation',
  'DeleteFinancialAnnotation',
  'FinancialAnnotationSubscription',
  // Admin: they read crawl tables but need an admin role (`AdminSystemRead`,
  // `AdminCrawlersManage`), so without a token they would always fail.
  'GetCrawlerStatus',
  'TriggerCrawl',
]);

export interface GraphQLResponse<T = any> {
  data?: T;
  errors?: Array<{
    message: string;
    locations?: Array<{ line: number; column: number }>;
    path?: string[];
    extensions?: { code?: string; [key: string]: unknown };
  }>;
}

export interface GraphQLRequest {
  query: string;
  variables?: Record<string, any>;
  operationName?: string;
}

/**
 * Thrown by `executeGraphQL` when the response carries GraphQL errors: the first error's
 * message, and its `extensions.code` when it has one.
 */
export class GraphQLResponseError extends Error {
  readonly code?: string;

  constructor(message: string, code?: string) {
    super(message);
    this.name = 'GraphQLResponseError';
    this.code = code;
  }
}

/**
 * `extensions.code` of the error a write returns when the backend couldn't ask the data plane
 * whether its series exists. Nothing was written, so the same request can be retried.
 */
export const DATA_PLANE_UNAVAILABLE = 'DATA_PLANE_UNAVAILABLE';

/** What the user is told for a `DATA_PLANE_UNAVAILABLE` error. */
export const DATA_PLANE_UNAVAILABLE_MESSAGE = "Couldn't check the series right now; try again";

/** Shown, and thrown by mutations, once the backend reports the account suspended. */
export const ACCOUNT_SUSPENDED_MESSAGE = 'This account is suspended';

/**
 * POST one GraphQL request, with the access token when there is one.
 * @param endpoint - The URL to POST to: the GraphQL URL, or the data URL for data operations.
 * @param request - The GraphQL request.
 * @param token - The access token, or null for an anonymous request.
 * @param signal - Optional AbortSignal passed to fetch.
 * @returns The fetch response.
 */
async function postGraphQL(
  endpoint: string,
  request: GraphQLRequest,
  token: string | null,
  signal?: AbortSignal
): Promise<Response> {
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    Accept: 'application/json',
  };
  // Signed-in requests carry the Keycloak access token; anonymous ones carry none.
  if (token) {
    headers.Authorization = `Bearer ${token}`;
  }
  return fetch(endpoint, {
    method: 'POST',
    headers,
    body: JSON.stringify(request),
    signal,
  });
}

/**
 * Whether the backend refused the request because the caller's account is suspended.
 * @param response - The fetch response; its body is read only when the status is 403.
 * @returns True for a 403 whose error code is ACCOUNT_SUSPENDED.
 */
async function isSuspendedRefusal(response: Response): Promise<boolean> {
  if (response.status !== 403) {
    return false;
  }
  try {
    const body: { errors?: Array<{ extensions?: { code?: string } }> } = await response
      .clone()
      .json();
    return body.errors?.some(error => error.extensions?.code === 'ACCOUNT_SUSPENDED') ?? false;
  } catch {
    return false;
  }
}

/** A top-level definition in a GraphQL document: its keyword (or `{`) and its name, if any. */
interface TopLevelDefinition {
  keyword: string;
  name: string | null;
}

const NAME_START = /[A-Za-z_]/;
const NAME_CONTINUE = /\w/;

/**
 * Lists a GraphQL document's top-level definitions without a full parse (`graphql` is only a
 * dev dependency, so the bundle doesn't carry its parser). Comments and string values are
 * skipped, and only words at brace depth 0 count, so a field called `query` inside a selection
 * set is not taken for an operation. Object values in a header's arguments (`$f: F = {a: 1}`,
 * `@d(a: {b: 1})`) are nested values, not the selection set.
 * @param document - The GraphQL document text.
 * @returns Its top-level definitions in order; `{` stands for an anonymous shorthand query.
 */
function topLevelDefinitions(document: string): TopLevelDefinition[] {
  const definitions: TopLevelDefinition[] = [];
  let depth = 0;
  // Open parentheses at brace depth 0: variable definitions or directive arguments.
  let parens = 0;
  let i = 0;
  // The definition whose header (keyword, name, variables, type condition, directives) is
  // being read, until its selection set opens.
  let header: TopLevelDefinition | null = null;
  // Whether the next word in the header is the definition's name.
  let nameNext = false;
  while (i < document.length) {
    const ch = document[i];
    if (ch === '#') {
      // A comment ends at a line terminator: `\n` or `\r` (GraphQL accepts a lone `\r`).
      while (i < document.length && document[i] !== '\n' && document[i] !== '\r') i++;
      continue;
    }
    if (document.startsWith('"""', i)) {
      // A block string ends at the first `"""` that is not escaped as `\"""`.
      i += 3;
      while (i < document.length && !document.startsWith('"""', i)) {
        i += document.startsWith('\\"""', i) ? 4 : 1;
      }
      i += 3;
      continue;
    }
    if (ch === '"') {
      i++;
      while (i < document.length && document[i] !== '"' && document[i] !== '\n') {
        i += document[i] === '\\' ? 2 : 1;
      }
      i++;
      continue;
    }
    if (NAME_START.test(ch)) {
      const start = i;
      while (i < document.length && NAME_CONTINUE.test(document[i])) i++;
      const word = document.slice(start, i);
      if (depth === 0) {
        if (header === null) {
          header = { keyword: word, name: null };
          definitions.push(header);
          nameNext = true;
        } else if (nameNext) {
          header.name = word;
          nameNext = false;
        }
      }
      continue;
    }
    if (ch === '{') {
      // At the top level, outside parentheses, `{` opens a selection set: the header's, or with
      // no header an anonymous `{ ... }` query. Inside parentheses it opens an object value.
      if (depth === 0 && parens === 0) {
        if (header === null) definitions.push({ keyword: '{', name: null });
        header = null;
      }
      depth++;
    } else if (ch === '}') {
      depth = Math.max(0, depth - 1);
    } else if (depth === 0 && ch === '(') {
      parens++;
      nameNext = false;
    } else if (depth === 0 && ch === ')') {
      parens = Math.max(0, parens - 1);
    } else if (depth === 0 && !/[\s,]/.test(ch)) {
      // Variables, directives or punctuation: a name can no longer follow.
      nameNext = false;
    }
    i++;
  }
  return definitions;
}

/**
 * Whether a request is a data operation, to be sent to the data URL without a token. Only a
 * document holding exactly one operation, a named query listed in `DATA_OPERATIONS`, counts,
 * and an explicit `operationName` must name that query. Everything else (mutations,
 * subscriptions, unnamed, unknown or several operations) is an app operation.
 * @param request - The GraphQL request.
 * @returns True when the request is a data operation.
 */
export function isDataOperation(request: GraphQLRequest): boolean {
  const operations = topLevelDefinitions(request.query).filter(def => def.keyword !== 'fragment');
  if (operations.length !== 1) return false;
  const [operation] = operations;
  if (operation.keyword !== 'query' || !operation.name) return false;
  if (request.operationName != null && request.operationName !== operation.name) return false;
  return DATA_OPERATIONS.has(operation.name);
}

/**
 * Execute a GraphQL query against the backend. Data operations (`isDataOperation`) go to the
 * data URL; everything else goes to the GraphQL URL. User tokens are forwarded to the app
 * endpoint and to trusted data endpoints; separate cross-realm data planes use public grants.
 * @param request - The GraphQL request object containing query, variables, and operation name.
 * @param signal - Optional AbortSignal passed to fetch; aborting it rejects with an AbortError.
 * @returns Promise that resolves to the GraphQL response.
 */
export async function executeGraphQL<T = any>(
  request: GraphQLRequest,
  signal?: AbortSignal
): Promise<GraphQLResponse<T>> {
  // Debug logging disabled for browser compatibility
  // if (MSW_DEBUG) {
  //   console.log('🔧 executeGraphQL called with:', { endpoint: GRAPHQL_ENDPOINT, request });
  // }
  const isData = isDataOperation(request);
  const endpoint = isData ? DATA_GRAPHQL_ENDPOINT : GRAPHQL_ENDPOINT;
  // Cross-realm branch environments deliberately use the shared plane's public grants.
  // Opt in only when the separate data plane trusts this frontend's issuer and audience.
  const anonymousDataPlane =
    isData &&
    DATA_GRAPHQL_ENDPOINT !== GRAPHQL_ENDPOINT &&
    import.meta.env.VITE_DATA_GRAPHQL_AUTH !== 'true';
  // Suspension preserves only anonymous catalog access. Expired/unverifiable credentials
  // otherwise fail normally; a denied catalog role is never retried anonymously.
  const isMutation = /^\s*mutation\b/.test(request.query);
  if (isAccountSuspended() && isMutation) {
    throw new Error(ACCOUNT_SUSPENDED_MESSAGE);
  }
  const token = isAccountSuspended() || anonymousDataPlane ? null : await getAccessToken();
  let response = await postGraphQL(endpoint, request, token, signal);

  if (token && (await isSuspendedRefusal(response))) {
    // A separate data backend may have an independent account database. Its refusal
    // must not disable mutations against the app backend.
    if (endpoint === GRAPHQL_ENDPOINT) setAccountSuspended(true);
    if (isMutation) {
      throw new Error(ACCOUNT_SUSPENDED_MESSAGE);
    }
    response = await postGraphQL(endpoint, request, null, signal);
  }

  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }

  const result: GraphQLResponse<T> = await response.json();

  if (result.errors && result.errors.length > 0) {
    console.error('GraphQL errors:', result.errors);
    const [first] = result.errors;
    throw new GraphQLResponseError(first.message, first.extensions?.code);
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
          externalId
          title
          description
          sourceId
          source {
            name
          }
          frequency
          units
          startDate
          endDate
          lastUpdated
          lifecycle { status since successorId proposedSuccessorId }
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
        externalId
        title
        description
        source {
          name
          notice {
            citation
            notice
            termsUrl
            retrievalDateRequired
          }
        }
        frequency
        units
        seasonalAdjustment
        startDate
        endDate
        lastUpdated
        isActive
        lifecycle { status since successorId proposedSuccessorId }
      }
    }
  `,

  // One page of series observations, transformed on the backend. Pass the previous page's
  // endCursor as `after` until hasNextPage is false.
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
      $frequency: SeriesFrequency
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
          externalId
          title
          description
          sourceId
          source {
            name
          }
          frequency
          units
          startDate
          endDate
          lastUpdated
          lifecycle { status since successorId proposedSuccessorId }
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
        lifecycleCounts { discontinued missingFromCatalog retired superseded }
        createdAt
        updatedAt
        notice {
          citation
          notice
          termsUrl
          retrievalDateRequired
        }
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
  // The backend returns public annotations, plus the caller's own private ones when the
  // request carries a signed-in user's token.
  GET_ANNOTATIONS_FOR_SERIES: `
    query GetAnnotationsForSeries($seriesId: String!) {
      annotationsForSeries(seriesId: $seriesId) {
        id
        userId
        annotationDate
        title
        description
        color
        visibility
      }
    }
  `,

  // Comments on an annotation the caller can see, oldest first.
  GET_COMMENTS_FOR_ANNOTATION: `
    query GetCommentsForAnnotation($annotationId: ID!) {
      commentsForAnnotation(annotationId: $annotationId) {
        id
        annotationId
        userId
        content
        createdAt
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

  GET_USER: `
    query GetUser($userId: ID!) {
      user(userId: $userId) {
        id
        email
        name
        avatarUrl
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
  // The author is the signed-in caller; the backend takes it from the token.
  CREATE_ANNOTATION: `
    mutation CreateAnnotation($input: CreateAnnotationInput!) {
      createAnnotation(input: $input) {
        id
        userId
        annotationDate
        title
        description
        color
        visibility
      }
    }
  `,

  // Only the annotation's author may update it.
  UPDATE_ANNOTATION: `
    mutation UpdateAnnotation($input: UpdateAnnotationInput!) {
      updateAnnotation(input: $input) {
        id
        userId
        annotationDate
        title
        description
        color
        visibility
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
        createdAt
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

// Publication state is separate from crawl health and observation dates.
export interface SeriesLifecycle {
  status:
    'active' | 'discontinued' | 'missing_from_catalog' | 'retired' | 'restricted' | 'superseded';
  since: string | null;
  successorId: string | null;
  proposedSuccessorId: string | null;
}

export interface SourceLifecycleCounts {
  discontinued: number;
  missingFromCatalog: number;
  retired: number;
  superseded: number;
}

// Type definitions for better TypeScript support
export interface SeriesListNode {
  lifecycle?: SeriesLifecycle | null;
  id: string;
  externalId: string;
  title: string;
  description: string;
  sourceId: string;
  source: {
    name: string;
  };
  frequency: string;
  units: string;
  startDate: string;
  endDate: string;
  lastUpdated: string;
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
    externalId: string;
    title: string;
    description: string | null;
    source: { name: string } | null;
    frequency: string;
    units: string | null;
    seasonalAdjustment: string | null;
    startDate: string | null;
    endDate: string | null;
    lastUpdated: string | null;
    isActive: boolean;
  } | null;
}

export interface DataPoint {
  date: string;
  /** BigDecimal on the wire: a numeric string, or null where the value is missing. */
  value: string | null;
  revisionDate: string;
  isOriginalRelease: boolean;
}

export interface SeriesDataResponse {
  seriesData: {
    nodes: DataPoint[];
    totalCount: number;
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
/** Who can see an annotation: only its author, or everyone. */
export type AnnotationVisibility = 'PRIVATE' | 'PUBLIC';

/** An annotation as `GetAnnotationsForSeries` and the annotation mutations select it. */
export interface ChartAnnotationType {
  id: string;
  /** Author's user id (the identity provider's `sub`). */
  userId: string;
  /** Observation date the note is about, `YYYY-MM-DD`. */
  annotationDate: string;
  title: string;
  description: string | null;
  /** Free text, up to 7 characters (default `#2196f3`); `useSeriesAnnotations` checks it. */
  color: string | null;
  visibility: AnnotationVisibility;
}

/** A comment as `GetCommentsForAnnotation` and `AddComment` select it. */
export interface AnnotationCommentType {
  id: string;
  annotationId: string;
  /** Author's user id (the identity provider's `sub`). */
  userId: string;
  content: string;
  /** RFC 3339 timestamp. */
  createdAt: string | null;
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

// Collaboration input types. The author is never sent; the backend takes it from the token.
export interface CreateAnnotationInput {
  seriesId: string;
  /** `YYYY-MM-DD`. */
  annotationDate: string;
  title: string;
  content: string;
  annotationType: string;
  color?: string;
  /** False (the default) keeps the annotation private to its author. */
  isPublic: boolean;
  /**
   * New for each annotation, reused only to retry the same create: the backend answers a
   * repeat with the annotation the first attempt stored instead of adding another (ECO-439).
   */
  idempotencyKey?: string;
}

/** Fields left out are unchanged. The date can't be changed. */
export interface UpdateAnnotationInput {
  annotationId: string;
  title?: string;
  content?: string;
  isPublic?: boolean;
}

export interface AddCommentInput {
  annotationId: string;
  content: string;
}

export interface ShareChartInput {
  owner_user_id: string;
  target_user_id: string;
  chart_id: string;
  permission_level: string;
}

export interface DeleteAnnotationInput {
  annotationId: string;
}

// Response types
export interface AnnotationsForSeriesResponse {
  annotationsForSeries: ChartAnnotationType[];
}

export interface CommentsForAnnotationResponse {
  commentsForAnnotation: AnnotationCommentType[];
}

export interface CreateAnnotationResponse {
  createAnnotation: ChartAnnotationType;
}

export interface UpdateAnnotationResponse {
  updateAnnotation: ChartAnnotationType;
}

export interface DeleteAnnotationResponse {
  deleteAnnotation: boolean;
}

export interface AddCommentResponse {
  addComment: AnnotationCommentType;
}

export interface ChartCollaboratorsResponse {
  chartCollaborators: ChartCollaboratorType[];
}

export interface UserResponse {
  user: UserType | null;
}
