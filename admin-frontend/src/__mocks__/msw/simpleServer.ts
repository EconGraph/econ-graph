/**
 * Simplified MSW server setup to avoid import issues
 * 
 * This version uses dynamic imports and simpler setup to avoid
 * TextEncoder issues in the Jest environment.
 */

// Mock scenarios for testing different states
export enum MockScenarios {
  SUCCESS = 'success',
  ERROR = 'error',
  LOADING = 'loading',
  UNAUTHORIZED = 'unauthorized'
}

// Simple mock responses without complex MSW setup
export const mockResponses = {
  GetCrawlerConfig: {
    success: {
      data: {
        crawlerConfig: {
          global_enabled: true,
          max_workers: 5,
          queue_size_limit: 10000,
          default_timeout: 30,
          default_retry_attempts: 3,
          rate_limit_global: 10,
          schedule_frequency: "hourly",
          error_threshold: 5,
          maintenance_mode: false,
          created_at: "2025-01-01T00:00:00Z",
          updated_at: "2025-01-01T10:00:00Z"
        }
      }
    },
    error: {
      data: null,
      errors: [
        {
          message: "Failed to retrieve crawler configuration",
          extensions: {
            code: "INTERNAL_ERROR",
            details: "Database connection timeout"
          }
        }
      ]
    }
  },
  
  GetDataSources: {
    success: {
      data: {
        dataSources: [
          {
            id: "fred",
            name: "Federal Reserve Economic Data (FRED)",
            enabled: true,
            priority: 1,
            rate_limit: 5,
            retry_attempts: 3,
            timeout_seconds: 30,
            last_success: "2025-01-01T09:50:00Z",
            last_error: null,
            health_status: "healthy",
            base_url: "https://api.stlouisfed.org/fred",
            api_key_required: true,
            created_at: "2025-01-01T00:00:00Z",
            updated_at: "2025-01-01T09:50:00Z"
          },
          {
            id: "bls",
            name: "Bureau of Labor Statistics (BLS)",
            enabled: true,
            priority: 2,
            rate_limit: 3,
            retry_attempts: 3,
            timeout_seconds: 45,
            last_success: "2025-01-01T09:35:00Z",
            last_error: null,
            health_status: "healthy",
            base_url: "https://api.bls.gov/publicAPI",
            api_key_required: true,
            created_at: "2025-01-01T00:00:00Z",
            updated_at: "2025-01-01T09:35:00Z"
          },
          {
            id: "census",
            name: "US Census Bureau",
            enabled: true,
            priority: 3,
            rate_limit: 2,
            retry_attempts: 5,
            timeout_seconds: 60,
            last_success: "2025-01-01T09:15:00Z",
            last_error: "Connection timeout",
            health_status: "warning",
            base_url: "https://api.census.gov/data",
            api_key_required: true,
            created_at: "2025-01-01T00:00:00Z",
            updated_at: "2025-01-01T09:15:00Z"
          },
          {
            id: "worldbank",
            name: "World Bank",
            enabled: false,
            priority: 4,
            rate_limit: 1,
            retry_attempts: 3,
            timeout_seconds: 30,
            last_success: null,
            last_error: "API key expired",
            health_status: "error",
            base_url: "https://api.worldbank.org/v2",
            api_key_required: false,
            created_at: "2025-01-01T00:00:00Z",
            updated_at: "2025-01-01T08:00:00Z"
          }
        ]
      }
    },
    error: {
      data: null,
      errors: [
        {
          message: "Failed to retrieve data sources",
          extensions: {
            code: "INTERNAL_ERROR",
            details: "Database connection timeout"
          }
        }
      ]
    }
  },
  
  getCrawlerStatus: {
    running: {
      crawlerStatus: {
        is_running: true,
        active_workers: 3,
        last_crawl: "2024-01-15T10:30:00Z",
        next_scheduled_crawl: "2024-01-15T11:00:00Z"
      }
    },
    stopped: {
      crawlerStatus: {
        is_running: false,
        active_workers: 0,
        last_crawl: "2024-01-15T09:30:00Z",
        next_scheduled_crawl: "2024-01-15T11:00:00Z"
      }
    },
    error: {
      data: null,
      errors: [
        {
          message: "Failed to retrieve crawler status",
          extensions: {
            code: "INTERNAL_ERROR",
            details: "Database connection timeout"
          }
        }
      ]
    }
  },
  
  updateCrawlerConfig: {
    success: {
      updateCrawlerConfig: {
        success: true,
        message: "Configuration updated successfully",
        config: {
          max_workers: 5,
          queue_size_limit: 10000,
          default_timeout_seconds: 30,
          default_retry_attempts: 3,
          schedule_frequency: "hourly",
          maintenance_mode: false,
          global_rate_limit: 1000,
          created_at: "2024-01-01T00:00:00Z",
          updated_at: "2024-01-15T10:45:00Z"
        }
      }
    },
    error: {
      data: null,
      errors: [
        {
          message: "Invalid configuration: max_workers cannot exceed 10",
          extensions: {
            code: "VALIDATION_ERROR",
            field: "max_workers",
            value: 15
          }
        }
      ]
    },
    unauthorized: {
      data: null,
      errors: [
        {
          message: "Insufficient permissions to update crawler configuration",
          extensions: {
            code: "FORBIDDEN",
            required_role: "super_admin",
            user_role: "admin"
          }
        }
      ]
    }
  }
};

// Simple scenario management
let currentScenario = 'default';

export const setMockScenario = (scenario: string) => {
  currentScenario = scenario;
  console.log(`[Simple MSW] Set mock scenario to: ${scenario}`);
};

export const resetMockScenario = () => {
  currentScenario = 'default';
  console.log(`[Simple MSW] Reset mock scenario to default`);
};

// Simple GraphQL response handler
export const getMockResponse = (operationName: string, scenario: string = currentScenario) => {
  console.log(`[Simple MSW] Looking for operation: ${operationName} with scenario: ${scenario}`);
  console.log(`[Simple MSW] Available operations:`, Object.keys(mockResponses));
  
  const responses = mockResponses[operationName as keyof typeof mockResponses];
  if (!responses) {
    console.error(`[Simple MSW] Unknown operation: ${operationName}`);
    return { data: null, errors: [{ message: `Unknown operation: ${operationName}` }] };
  }
  
  console.log(`[Simple MSW] Available scenarios for ${operationName}:`, Object.keys(responses));
  
  const response = responses[scenario as keyof typeof responses];
  if (!response) {
    console.error(`[Simple MSW] Unknown scenario: ${scenario} for operation: ${operationName}`);
    return { data: null, errors: [{ message: `Unknown scenario: ${scenario}` }] };
  }
  
  console.log(`[Simple MSW] Found response for ${operationName}/${scenario}`);
  return response;
};

// Mock fetch to intercept GraphQL requests
// Store original fetch to properly restore it
let originalFetch: typeof global.fetch;

export const setupSimpleMSW = () => {
  console.log('[Simple MSW] Setting up MSW...');
  console.log('[Simple MSW] Original fetch type:', typeof global.fetch);
  originalFetch = global.fetch;
  
  global.fetch = jest.fn((url: string, options?: any) => {
    console.log(`[Simple MSW] ===== FETCH INTERCEPTED =====`);
    console.log(`[Simple MSW] URL: "${url}"`);
    console.log(`[Simple MSW] Method: ${options?.method || 'GET'}`);
    console.log(`[Simple MSW] Headers:`, options?.headers);
    console.log(`[Simple MSW] Body:`, options?.body);
    console.log(`[Simple MSW] ============================`);
    
    // Intercept GraphQL requests - be more flexible with URL matching
    if (url.includes('graphql') || url === '/graphql' || url.endsWith('/graphql')) {
      console.log(`[Simple MSW] *** GRAPHQL REQUEST DETECTED ***`);
      try {
        const body = JSON.parse(options.body);
        const operationName = body.operationName;
        const variables = body.variables || {};
        
        console.log(`[Simple MSW] Operation: ${operationName}`);
        console.log(`[Simple MSW] Variables:`, variables);
        console.log(`[Simple MSW] Current scenario: ${currentScenario}`);
        
        const response = getMockResponse(operationName, currentScenario);
        
        console.log(`[Simple MSW] Returning mock response:`, response);
        
        return Promise.resolve({
          ok: true,
          json: () => Promise.resolve(response),
          status: 200,
          statusText: 'OK'
        } as Response);
      } catch (error) {
        console.error('[Simple MSW] Error parsing GraphQL request:', error);
        console.error('[Simple MSW] Request body:', options.body);
        return originalFetch(url, options);
      }
    }
    
    // For non-GraphQL requests, pass through to original fetch
    console.log(`[Simple MSW] Passing through request: ${url}`);
    return originalFetch(url, options);
  }) as jest.MockedFunction<typeof fetch>;
  
  console.log('[Simple MSW] MSW setup complete');
};

export const cleanupSimpleMSW = () => {
  // Properly restore original fetch
  if (originalFetch) {
    global.fetch = originalFetch;
  }
};
