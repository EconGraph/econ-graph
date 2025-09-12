/**
 * Security Functions for Chart API Service
 *
 * Provides network security and access control for the private chart API.
 */

/**
 * Check if request is from internal network (backend/MCP server)
 */
function isInternalNetworkRequest(clientIP, req) {
  // Check for internal network IPs
  const internalIPs = [
    '127.0.0.1',      // localhost
    '::1',            // IPv6 localhost
    '10.0.0.0/8',     // Private network
    '172.16.0.0/12',  // Private network
    '192.168.0.0/16', // Private network
  ];

  // Check if IP is in internal ranges
  for (const internalIP of internalIPs) {
    if (clientIP === internalIP || isIPInRange(clientIP, internalIP)) {
      return true;
    }
  }

  // Check for specific headers that indicate internal request
  // ALL required headers must be present for security
  const requiredHeaders = [
    'x-mcp-server-request',
    'x-internal-request'
  ];

  for (const header of requiredHeaders) {
    if (req.headers[header] !== 'true') {
      return false;
    }
  }

  // If all required headers are present, allow the request
  return true;
}

/**
 * Check if request is from Kubernetes internal network
 */
function isKubernetesInternalRequest(clientIP, req) {
  // Kubernetes internal IP ranges
  const k8sInternalRanges = [
    '10.0.0.0/8',     // Kubernetes default pod network
    '172.16.0.0/12',  // Kubernetes default service network
    '192.168.0.0/16', // Kubernetes default node network
  ];

  // Check if IP is in Kubernetes internal ranges
  for (const range of k8sInternalRanges) {
    if (isIPInRange(clientIP, range)) {
      return true;
    }
  }

  // Check for Kubernetes service discovery headers
  const k8sHeaders = [
    'x-forwarded-for',
    'x-real-ip',
    'x-kubernetes-service-account'
  ];

  for (const header of k8sHeaders) {
    if (req.headers[header]) {
      return true;
    }
  }

  return false;
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

/**
 * Validate security headers
 */
function validateSecurityHeaders(req) {
  const requiredHeaders = [
    'x-mcp-server-request',
    'x-internal-request'
  ];

  for (const header of requiredHeaders) {
    if (req.headers[header] !== 'true') {
      return false;
    }
  }

  return true;
}

/**
 * Get client IP address from request
 */
function getClientIP(req) {
  return req.ip ||
         req.connection.remoteAddress ||
         req.socket.remoteAddress ||
         (req.connection.socket ? req.connection.socket.remoteAddress : null) ||
         req.headers['x-forwarded-for']?.split(',')[0]?.trim() ||
         '127.0.0.1';
}

module.exports = {
  isInternalNetworkRequest,
  isKubernetesInternalRequest,
  isIPInRange,
  ipToNumber,
  validateSecurityHeaders,
  getClientIP
};
