/**
 * Private API Routes for MCP Server Integration
 *
 * These routes are only accessible from the backend/MCP server within the private network.
 * They provide chart generation capabilities for the MCP server.
 */

import React from 'react';
import { Routes, Route } from 'react-router-dom';
// import { handlePrivateChartRequest, handleChartHealthCheck } from '../api/privateChartEndpoint';

/**
 * Private API Routes Component
 * This component handles private API endpoints that are only accessible from the MCP server
 */
export function PrivateApiRoutes() {
  return (
    <Routes>
      {/* Private Chart API Endpoints */}
      <Route path='/api/private/chart/generate' element={<PrivateChartEndpoint />} />
      <Route path='/api/private/chart/health' element={<PrivateChartHealthEndpoint />} />
    </Routes>
  );
}

/**
 * Private Chart Generation Endpoint Component
 */
function PrivateChartEndpoint() {
  React.useEffect(() => {
    // This would be handled by Express.js in a real implementation
    // For now, we'll create a mock endpoint that returns chart configuration
    const handleRequest = async (event: MessageEvent) => {
      if (event.data.type === 'CHART_REQUEST') {
        try {
          const { generateChartConfig, validateChartRequest } = await import('../api/chartApi');

          if (validateChartRequest(event.data.payload)) {
            const response = generateChartConfig(event.data.payload);

            // Send response back to MCP server
            window.postMessage(
              {
                type: 'CHART_RESPONSE',
                payload: response,
              },
              '*'
            );
          } else {
            window.postMessage(
              {
                type: 'CHART_RESPONSE',
                payload: {
                  success: false,
                  error: 'Invalid chart request',
                },
              },
              '*'
            );
          }
        } catch (error) {
          window.postMessage(
            {
              type: 'CHART_RESPONSE',
              payload: {
                success: false,
                error: error instanceof Error ? error.message : 'Unknown error',
              },
            },
            '*'
          );
        }
      }
    };

    window.addEventListener('message', handleRequest);
    return () => window.removeEventListener('message', handleRequest);
  }, []);

  return (
    <div style={{ display: 'none' }}>
      Private Chart API Endpoint - Only accessible from MCP server
    </div>
  );
}

/**
 * Private Chart Health Check Endpoint Component
 */
function PrivateChartHealthEndpoint() {
  React.useEffect(() => {
    const handleHealthCheck = async (event: MessageEvent) => {
      if (event.data.type === 'HEALTH_CHECK') {
        window.postMessage(
          {
            type: 'HEALTH_RESPONSE',
            payload: {
              success: true,
              message: 'Private chart API is healthy',
              timestamp: new Date().toISOString(),
              version: '1.0.0',
            },
          },
          '*'
        );
      }
    };

    window.addEventListener('message', handleHealthCheck);
    return () => window.removeEventListener('message', handleHealthCheck);
  }, []);

  return (
    <div style={{ display: 'none' }}>
      Private Chart Health Check - Only accessible from MCP server
    </div>
  );
}

/**
 * Mock Express.js handler for development
 * In production, this would be handled by the actual Express.js server
 */
export const mockPrivateChartHandler = {
  generate: async (request: any) => {
    const { generateChartConfig, validateChartRequest } = await import('../api/chartApi');

    if (!validateChartRequest(request)) {
      return {
        success: false,
        error:
          'Invalid chart request. Required fields: seriesData (array), chartType (line|bar|scatter)',
      };
    }

    return generateChartConfig(request);
  },

  health: async () => {
    return {
      success: true,
      message: 'Private chart API is healthy',
      timestamp: new Date().toISOString(),
      version: '1.0.0',
    };
  },
};
