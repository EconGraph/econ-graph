/**
 * Private Chart API Server for MCP Integration
 *
 * This Express.js server provides private chart generation endpoints
 * that are only accessible from the backend/MCP server within the private network.
 */

const express = require('express');
const cors = require('cors');
const { generateChartConfig, validateChartRequest } = require('../api/chartApi.cjs');

const app = express();
const PORT = process.env.PRIVATE_CHART_API_PORT || 3001;

// Middleware
app.use(express.json({ limit: '10mb' }));
app.use(
  cors({
    origin: false, // Disable CORS for security - only internal requests
    credentials: true,
  })
);

// Security middleware - only allow internal requests
app.use((req, res, next) => {
  const clientIP = req.ip || req.connection.remoteAddress;
  const isInternalRequest = isInternalNetworkRequest(clientIP, req);

  if (!isInternalRequest) {
    return res.status(403).json({
      success: false,
      error: 'Access denied: This endpoint is only accessible from the MCP server',
    });
  }

  next();
});

/**
 * Private chart generation endpoint
 * POST /api/private/chart/generate
 */
app.post('/api/private/chart/generate', async (req, res) => {
  try {
    const chartRequest = req.body;

    // Validate request
    if (!validateChartRequest(chartRequest)) {
      return res.status(400).json({
        success: false,
        error:
          'Invalid chart request. Required fields: seriesData (array), chartType (line|bar|scatter)',
      });
    }

    // Generate chart configuration
    const chartResponse = generateChartConfig(chartRequest);

    if (!chartResponse.success) {
      return res.status(400).json(chartResponse);
    }

    // Return successful response
    res.status(200).json({
      success: true,
      chartConfig: chartResponse.config,
      chartData: chartResponse.config,
      metadata: chartResponse.metadata,
      message: 'Chart configuration generated successfully',
    });
  } catch (error) {
    console.error('Private chart API error:', error);
    res.status(500).json({
      success: false,
      error: 'Internal server error while generating chart',
    });
  }
});

// Handle GET requests to chart generation endpoint (should return 405)
app.get('/api/private/chart/generate', (req, res) => {
  res.status(405).json({
    success: false,
    error: 'Method not allowed. Use POST for chart generation.',
  });
});

/**
 * Health check endpoint
 * GET /api/private/chart/health
 */
app.get('/api/private/chart/health', (req, res) => {
  res.status(200).json({
    success: true,
    message: 'Private chart API is healthy',
    timestamp: new Date().toISOString(),
    version: '1.0.0',
  });
});

/**
 * Check if request is from internal network (backend/MCP server)
 */
function isInternalNetworkRequest(clientIP, req) {
  // Check for internal network IPs
  const internalIPs = [
    '127.0.0.1', // localhost
    '::1', // IPv6 localhost
    '10.0.0.0/8', // Private network
    '172.16.0.0/12', // Private network
    '192.168.0.0/16', // Private network
  ];

  // Check if IP is in internal ranges
  let isInternalIP = false;
  for (const internalIP of internalIPs) {
    if (clientIP === internalIP || isIPInRange(clientIP, internalIP)) {
      isInternalIP = true;
      break;
    }
  }

  // Check for specific headers that indicate internal request
  const internalHeaders = ['x-mcp-server-request', 'x-internal-request', 'x-backend-request'];
  let hasInternalHeader = false;

  for (const header of internalHeaders) {
    if (req.headers[header] === 'true') {
      hasInternalHeader = true;
      break;
    }
  }

  // In test environment, allow requests with proper headers
  if (process.env.NODE_ENV === 'test') {
    return hasInternalHeader;
  }

  // In development, allow localhost without headers for easier testing
  if (process.env.NODE_ENV === 'development' && clientIP === '127.0.0.1') {
    return true;
  }

  // In production, require both IP and header
  return isInternalIP && hasInternalHeader;
}

/**
 * Check if IP is in CIDR range
 */
function isIPInRange(ip, cidr) {
  if (!cidr.includes('/')) {
    return ip === cidr;
  }

  const [range, bits] = cidr.split('/');
  const mask = -1 << (32 - parseInt(bits));

  const ipNum = ipToNumber(ip);
  const rangeNum = ipToNumber(range);

  return (ipNum & mask) === (rangeNum & mask);
}

/**
 * Convert IP address to number
 */
function ipToNumber(ip) {
  return ip.split('.').reduce((acc, octet) => (acc << 8) + parseInt(octet), 0);
}

// Start server
if (require.main === module) {
  app.listen(PORT, '127.0.0.1', () => {
    console.log(`🔒 Private Chart API Server running on http://127.0.0.1:${PORT}`);
    console.log(`📊 Available endpoints:`);
    console.log(`   POST /api/private/chart/generate - Generate chart configuration`);
    console.log(`   GET  /api/private/chart/health   - Health check`);
    console.log(`🔐 Security: Only accessible from MCP server (internal network)`);
  });
}

module.exports = app;
