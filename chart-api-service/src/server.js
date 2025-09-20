/**
 * Private Chart API Service for EconGraph MCP Server Integration
 *
 * This is a standalone Express.js service that provides secure chart generation
 * capabilities for the MCP (Model Context Protocol) server. It runs as a separate
 * service with its own Kubernetes deployment and internal-only network access.
 */

const express = require('express');
const cors = require('cors');
const helmet = require('helmet');
const rateLimit = require('express-rate-limit');
const compression = require('compression');
const morgan = require('morgan');
require('dotenv').config();

const { generateChartConfig, validateChartRequest } = require('./chartApi');
const { isInternalNetworkRequest } = require('./security');

const app = express();
const PORT = process.env.PORT || 3001;
const NODE_ENV = process.env.NODE_ENV || 'development';

// Security middleware
app.use(helmet({
  contentSecurityPolicy: {
    directives: {
      defaultSrc: ["'self'"],
      scriptSrc: ["'self'"],
      styleSrc: ["'self'", "'unsafe-inline'"],
      imgSrc: ["'self'", "data:", "https:"],
      connectSrc: ["'self'"],
      fontSrc: ["'self'"],
      objectSrc: ["'none'"],
      mediaSrc: ["'self'"],
      frameSrc: ["'none'"],
    },
  },
  crossOriginEmbedderPolicy: false
}));

// Rate limiting
const limiter = rateLimit({
  windowMs: 15 * 60 * 1000, // 15 minutes
  max: 1000, // Limit each IP to 1000 requests per windowMs
  message: {
    success: false,
    error: 'Too many requests from this IP, please try again later.'
  },
  standardHeaders: true,
  legacyHeaders: false,
});
app.use(limiter);

// Compression
app.use(compression());

// Logging
if (NODE_ENV === 'production') {
  app.use(morgan('combined'));
} else {
  app.use(morgan('dev'));
}

// Body parsing
app.use(express.json({ limit: '10mb' }));
app.use(express.urlencoded({ extended: true, limit: '10mb' }));

// CORS - Disabled for security (internal service only)
app.use(cors({
  origin: false,
  credentials: false
}));

// Security middleware - only allow internal requests
app.use((req, res, next) => {
  const clientIP = req.ip || req.connection.remoteAddress || req.socket.remoteAddress;
  const isInternalRequest = isInternalNetworkRequest(clientIP, req);

  if (!isInternalRequest) {
    return res.status(403).json({
      success: false,
      error: 'Access denied: This service is only accessible from internal network',
      timestamp: new Date().toISOString()
    });
  }

  next();
});

/**
 * Health check endpoint
 * GET /health
 */
app.get('/health', (req, res) => {
  res.status(200).json({
    success: true,
    message: 'Chart API Service is healthy',
    timestamp: new Date().toISOString(),
    version: '1.0.0',
    environment: NODE_ENV,
    uptime: process.uptime()
  });
});

/**
 * Chart generation endpoint
 * POST /api/chart/generate
 */
app.post('/api/chart/generate', async (req, res) => {
  try {
    const chartRequest = req.body;

    // Validate request
    if (!validateChartRequest(chartRequest)) {
      return res.status(400).json({
        success: false,
        error: 'Invalid chart request. Required fields: seriesData (array), chartType (line|bar|scatter)',
        timestamp: new Date().toISOString()
      });
    }

    // Generate chart configuration
    const chartResponse = generateChartConfig(chartRequest);

    if (!chartResponse.success) {
      return res.status(400).json({
        ...chartResponse,
        timestamp: new Date().toISOString()
      });
    }

    // Return successful response
    res.status(200).json({
      success: true,
      chartConfig: chartResponse.chartConfig,
      chartData: chartResponse.chartData,
      metadata: chartResponse.metadata,
      message: 'Chart configuration generated successfully',
      timestamp: new Date().toISOString()
    });

  } catch (error) {
    console.error('Chart API error:', error);
    res.status(500).json({
      success: false,
      error: 'Internal server error while generating chart',
      timestamp: new Date().toISOString()
    });
  }
});

/**
 * Chart types endpoint
 * GET /api/chart/types
 */
app.get('/api/chart/types', (req, res) => {
  res.status(200).json({
    success: true,
    chartTypes: [
      {
        type: 'line',
        description: 'Line charts for time series data and trends',
        supportedFeatures: ['multiple series', 'time scale', 'smooth curves']
      },
      {
        type: 'bar',
        description: 'Bar charts for categorical comparisons',
        supportedFeatures: ['grouped bars', 'stacked bars', 'categorical data']
      },
      {
        type: 'scatter',
        description: 'Scatter plots for correlation analysis',
        supportedFeatures: ['point markers', 'trend lines', 'correlation analysis']
      }
    ],
    timestamp: new Date().toISOString()
  });
});

/**
 * Service info endpoint
 * GET /api/info
 */
app.get('/api/info', (req, res) => {
  res.status(200).json({
    success: true,
    service: 'EconGraph Chart API',
    version: '1.0.0',
    description: 'Private chart generation service for MCP server integration',
    endpoints: [
      'GET /health - Health check',
      'POST /api/chart/generate - Generate chart configuration',
      'GET /api/chart/types - Get supported chart types',
      'GET /api/info - Service information'
    ],
    security: {
      internalNetworkOnly: true,
      requiredHeaders: ['X-MCP-Server-Request', 'X-Internal-Request'],
      rateLimit: '1000 requests per 15 minutes'
    },
    timestamp: new Date().toISOString()
  });
});

// 404 handler
app.use('*', (req, res) => {
  res.status(404).json({
    success: false,
    error: 'Endpoint not found',
    availableEndpoints: [
      'GET /health',
      'POST /api/chart/generate',
      'GET /api/chart/types',
      'GET /api/info'
    ],
    timestamp: new Date().toISOString()
  });
});

// Error handler
app.use((error, req, res, next) => {
  // Don't log JSON parse errors as they're expected in tests
  if (error.type !== 'entity.parse.failed') {
    console.error('Unhandled error:', error);
  }

  // Handle JSON parse errors
  if (error.type === 'entity.parse.failed') {
    return res.status(400).json({
      success: false,
      error: 'Invalid JSON in request body',
      timestamp: new Date().toISOString()
    });
  }

  res.status(500).json({
    success: false,
    error: 'Internal server error',
    timestamp: new Date().toISOString()
  });
});

// Start server
if (require.main === module) {
  app.listen(PORT, '0.0.0.0', () => {
    console.log(`🔒 Chart API Service running on port ${PORT}`);
    console.log(`📊 Available endpoints:`);
    console.log(`   GET  /health              - Health check`);
    console.log(`   POST /api/chart/generate  - Generate chart configuration`);
    console.log(`   GET  /api/chart/types     - Get supported chart types`);
    console.log(`   GET  /api/info            - Service information`);
    console.log(`🔐 Security: Internal network access only`);
    console.log(`🌍 Environment: ${NODE_ENV}`);
  });
}

module.exports = app;
