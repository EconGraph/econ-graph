/**
 * Private Chart API Endpoint for MCP Server.
 *
 * This endpoint is only accessible from the backend/MCP server within the private network.
 * It provides chart generation capabilities using the existing frontend charting components.
 */

import { Request, Response } from 'express';
import { generateChartConfig, validateChartRequest } from './chartApi';

/**
 * Private chart generation endpoint
 * Only accessible from backend/MCP server (IP whitelist or internal network).
 * @param req
 * @param res
 */
export async function handlePrivateChartRequest(req: Request, res: Response): Promise<void> {
  try {
    // Security check: Only allow requests from backend/MCP server
    const clientIP = req.ip || req.connection.remoteAddress;
    const isInternalRequest = isInternalNetworkRequest(clientIP || '', req);

    if (!isInternalRequest) {
      res.status(403).json({
        success: false,
        error: 'Access denied: This endpoint is only accessible from the MCP server',
      });
      return;
    }

    // Validate request method
    if (req.method !== 'POST') {
      res.status(405).json({
        success: false,
        error: 'Method not allowed. Use POST.',
      });
      return;
    }

    // Parse and validate request body
    const chartRequest = req.body;

    if (!validateChartRequest(chartRequest)) {
      res.status(400).json({
        success: false,
        error:
          'Invalid chart request. Required fields: seriesData (array), chartType (line|bar|scatter)',
      });
      return;
    }

    // Generate chart configuration
    const chartResponse = generateChartConfig(chartRequest);

    if (!chartResponse.success) {
      res.status(400).json(chartResponse);
      return;
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
}

/**
 * Check if request is from internal network (backend/MCP server).
 * @param clientIP
 * @param req
 */
function isInternalNetworkRequest(clientIP: string, req: Request): boolean {
  // Check for internal network IPs
  const internalIPs = [
    '127.0.0.1', // localhost
    '::1', // IPv6 localhost
    '10.0.0.0/8', // Private network
    '172.16.0.0/12', // Private network
    '192.168.0.0/16', // Private network
  ];

  // Check if IP is in internal ranges
  for (const internalIP of internalIPs) {
    if (clientIP === internalIP || isIPInRange(clientIP, internalIP)) {
      return true;
    }
  }

  // Check for specific headers that indicate internal request
  const internalHeaders = ['x-mcp-server-request', 'x-internal-request', 'x-backend-request'];

  for (const header of internalHeaders) {
    if (req.headers[header] === 'true') {
      return true;
    }
  }

  // Check if request is from localhost (development)
  if (process.env.NODE_ENV === 'development' && clientIP === '127.0.0.1') {
    return true;
  }

  return false;
}

/**
 * Check if IP is in CIDR range.
 * @param ip
 * @param cidr
 */
function isIPInRange(ip: string, cidr: string): boolean {
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
 * Convert IP address to number.
 * @param ip
 */
function ipToNumber(ip: string): number {
  return ip.split('.').reduce((acc, octet) => (acc << 8) + parseInt(octet), 0);
}

/**
 * Health check endpoint for MCP server.
 * @param req
 * @param res
 */
export async function handleChartHealthCheck(req: Request, res: Response): Promise<void> {
  try {
    const clientIP = req.ip || req.connection.remoteAddress;
    const isInternalRequest = isInternalNetworkRequest(clientIP || '', req);

    if (!isInternalRequest) {
      res.status(403).json({
        success: false,
        error: 'Access denied',
      });
      return;
    }

    res.status(200).json({
      success: true,
      message: 'Private chart API is healthy',
      timestamp: new Date().toISOString(),
      version: '1.0.0',
    });
  } catch (error) {
    res.status(500).json({
      success: false,
      error: 'Health check failed',
    });
  }
}
