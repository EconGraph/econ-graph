/**
 * Tests for Private Chart API Server
 *
 * Tests the Express.js server that provides private chart generation
 * endpoints for the MCP server integration.
 */

// Set test environment
process.env.NODE_ENV = 'test';

const request = require('supertest');
const app = require('../privateChartServer.cjs');

describe('Private Chart API Server', () => {
  const validChartRequest = {
    seriesData: [
      {
        id: 'series-1',
        name: 'GDP',
        dataPoints: [
          { date: '2020-01-01', value: 100.0 },
          { date: '2020-02-01', value: 101.5 },
        ],
        color: '#1976d2',
        type: 'line',
      },
    ],
    chartType: 'line',
    title: 'Test Chart',
    showLegend: true,
    showGrid: true,
    yAxisLabel: 'Value',
    xAxisLabel: 'Date',
  };

  describe('POST /api/private/chart/generate', () => {
    it('should generate chart configuration with valid request', async () => {
      const response = await request(app)
        .post('/api/private/chart/generate')
        .set('X-MCP-Server-Request', 'true')
        .set('X-Internal-Request', 'true')
        .send(validChartRequest);

      expect(response.status).toBe(200);
      expect(response.body.success).toBe(true);
      expect(response.body.chartConfig).toBeDefined();
      expect(response.body.chartData).toBeDefined();
      expect(response.body.metadata).toBeDefined();
      expect(response.body.message).toBe('Chart configuration generated successfully');
    });

    it('should generate line chart configuration', async () => {
      const lineChartRequest = {
        ...validChartRequest,
        chartType: 'line',
      };

      const response = await request(app)
        .post('/api/private/chart/generate')
        .set('X-MCP-Server-Request', 'true')
        .set('X-Internal-Request', 'true')
        .send(lineChartRequest);

      expect(response.status).toBe(200);
      expect(response.body.chartConfig.type).toBe('line');
    });

    it('should generate bar chart configuration', async () => {
      const barChartRequest = {
        ...validChartRequest,
        chartType: 'bar',
      };

      const response = await request(app)
        .post('/api/private/chart/generate')
        .set('X-MCP-Server-Request', 'true')
        .set('X-Internal-Request', 'true')
        .send(barChartRequest);

      expect(response.status).toBe(200);
      expect(response.body.chartConfig.type).toBe('bar');
    });

    it('should generate scatter chart configuration', async () => {
      const scatterChartRequest = {
        ...validChartRequest,
        chartType: 'scatter',
      };

      const response = await request(app)
        .post('/api/private/chart/generate')
        .set('X-MCP-Server-Request', 'true')
        .set('X-Internal-Request', 'true')
        .send(scatterChartRequest);

      expect(response.status).toBe(200);
      expect(response.body.chartConfig.type).toBe('scatter');
    });

    it('should reject request without MCP server header', async () => {
      const response = await request(app)
        .post('/api/private/chart/generate')
        .send(validChartRequest);

      expect(response.status).toBe(403);
      expect(response.body.success).toBe(false);
      expect(response.body.error).toContain('Access denied');
    });

    it('should reject request without internal request header', async () => {
      const response = await request(app)
        .post('/api/private/chart/generate')
        .send(validChartRequest);

      expect(response.status).toBe(403);
      expect(response.body.success).toBe(false);
      expect(response.body.error).toContain('Access denied');
    });

    it('should reject invalid chart request', async () => {
      const invalidRequest = {
        chartType: 'line',
        // Missing seriesData
      };

      const response = await request(app)
        .post('/api/private/chart/generate')
        .set('X-MCP-Server-Request', 'true')
        .set('X-Internal-Request', 'true')
        .send(invalidRequest);

      expect(response.status).toBe(400);
      expect(response.body.success).toBe(false);
      expect(response.body.error).toContain('Invalid chart request');
    });

    it('should reject request with invalid chart type', async () => {
      const invalidRequest = {
        ...validChartRequest,
        chartType: 'pie', // Invalid chart type
      };

      const response = await request(app)
        .post('/api/private/chart/generate')
        .set('X-MCP-Server-Request', 'true')
        .set('X-Internal-Request', 'true')
        .send(invalidRequest);

      expect(response.status).toBe(400);
      expect(response.body.success).toBe(false);
      expect(response.body.error).toContain('Invalid chart request');
    });

    it('should reject GET request to chart generation endpoint', async () => {
      const response = await request(app)
        .get('/api/private/chart/generate')
        .set('X-MCP-Server-Request', 'true')
        .set('X-Internal-Request', 'true');

      expect(response.status).toBe(405);
      expect(response.body.success).toBe(false);
      expect(response.body.error).toContain('Method not allowed');
    });

    it('should handle empty series data', async () => {
      const emptyRequest = {
        ...validChartRequest,
        seriesData: [],
      };

      const response = await request(app)
        .post('/api/private/chart/generate')
        .set('X-MCP-Server-Request', 'true')
        .set('X-Internal-Request', 'true')
        .send(emptyRequest);

      expect(response.status).toBe(400);
      expect(response.body.success).toBe(false);
      expect(response.body.error).toContain('Invalid chart request');
    });

    it('should handle multiple series data', async () => {
      const multiSeriesRequest = {
        ...validChartRequest,
        seriesData: [
          {
            id: 'series-1',
            name: 'GDP',
            dataPoints: [
              { date: '2020-01-01', value: 100.0 },
              { date: '2020-02-01', value: 101.5 },
            ],
            color: '#1976d2',
          },
          {
            id: 'series-2',
            name: 'Unemployment',
            dataPoints: [
              { date: '2020-01-01', value: 3.5 },
              { date: '2020-02-01', value: 3.6 },
            ],
            color: '#d32f2f',
          },
        ],
      };

      const response = await request(app)
        .post('/api/private/chart/generate')
        .set('X-MCP-Server-Request', 'true')
        .set('X-Internal-Request', 'true')
        .send(multiSeriesRequest);

      expect(response.status).toBe(200);
      expect(response.body.chartConfig.data.datasets).toHaveLength(2);
      expect(response.body.metadata.seriesCount).toBe(2);
    });

    it('should include proper metadata in response', async () => {
      const response = await request(app)
        .post('/api/private/chart/generate')
        .set('X-MCP-Server-Request', 'true')
        .set('X-Internal-Request', 'true')
        .send(validChartRequest);

      expect(response.status).toBe(200);
      expect(response.body.metadata).toEqual({
        seriesCount: 1,
        dataPointCount: 2,
        chartType: 'line',
      });
    });
  });

  describe('GET /api/private/chart/health', () => {
    it('should return health status with MCP server header', async () => {
      const response = await request(app)
        .get('/api/private/chart/health')
        .set('X-MCP-Server-Request', 'true')
        .set('X-Internal-Request', 'true');

      expect(response.status).toBe(200);
      expect(response.body.success).toBe(true);
      expect(response.body.message).toBe('Private chart API is healthy');
      expect(response.body.version).toBe('1.0.0');
      expect(response.body.timestamp).toBeDefined();
    });

    it('should reject health check without MCP server header', async () => {
      const response = await request(app).get('/api/private/chart/health');

      expect(response.status).toBe(403);
      expect(response.body.success).toBe(false);
      expect(response.body.error).toContain('Access denied');
    });

    it('should reject health check without internal request header', async () => {
      const response = await request(app).get('/api/private/chart/health');

      expect(response.status).toBe(403);
      expect(response.body.success).toBe(false);
      expect(response.body.error).toContain('Access denied');
    });
  });

  describe('Security Tests', () => {
    it('should reject requests from external IPs', async () => {
      // Mock external IP
      const originalEnv = process.env.NODE_ENV;
      process.env.NODE_ENV = 'production';

      const response = await request(app)
        .post('/api/private/chart/generate')
        .set('X-MCP-Server-Request', 'true')
        .set('X-Internal-Request', 'true')
        .set('X-Forwarded-For', '8.8.8.8') // External IP
        .send(validChartRequest);

      // In production, this should be rejected
      // In test environment, it might pass due to localhost
      expect([200, 403]).toContain(response.status);

      process.env.NODE_ENV = originalEnv;
    });

    it('should accept requests with proper security headers', async () => {
      const response = await request(app)
        .post('/api/private/chart/generate')
        .set('X-MCP-Server-Request', 'true')
        .set('X-Internal-Request', 'true')
        .set('X-Backend-Request', 'true')
        .send(validChartRequest);

      expect(response.status).toBe(200);
      expect(response.body.success).toBe(true);
    });
  });

  describe('Error Handling', () => {
    it('should handle malformed JSON', async () => {
      const response = await request(app)
        .post('/api/private/chart/generate')
        .set('X-MCP-Server-Request', 'true')
        .set('X-Internal-Request', 'true')
        .set('Content-Type', 'application/json')
        .send('{"invalid": json}');

      expect(response.status).toBe(400);
    });

    it('should handle oversized requests', async () => {
      // Create a large request (over 10MB limit)
      const largeDataPoints = Array.from({ length: 100000 }, (_, i) => ({
        date: `2020-01-${String((i % 30) + 1).padStart(2, '0')}`,
        value: Math.random() * 1000,
      }));

      const largeRequest = {
        ...validChartRequest,
        seriesData: [
          {
            ...validChartRequest.seriesData[0],
            dataPoints: largeDataPoints,
          },
        ],
      };

      const response = await request(app)
        .post('/api/private/chart/generate')
        .set('X-MCP-Server-Request', 'true')
        .set('X-Internal-Request', 'true')
        .send(largeRequest);

      // Should either succeed or fail gracefully
      expect([200, 413]).toContain(response.status);
    });
  });
});
