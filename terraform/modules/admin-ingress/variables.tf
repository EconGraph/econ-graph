# REQUIREMENT: Variables for secure admin ingress configuration
# PURPOSE: Configure admin access controls and security parameters
# This ensures proper isolation and security for administrative interface access

variable "admin_domain" {
  description = "Domain name for admin interface (e.g., admin.internal.company.com)"
  type        = string

  validation {
    condition     = can(regex("^[a-zA-Z0-9][a-zA-Z0-9-]{1,61}[a-zA-Z0-9]\\.[a-zA-Z]{2,}$", var.admin_domain))
    error_message = "Admin domain must be a valid domain name."
  }
}

variable "admin_namespace" {
  description = "Kubernetes namespace for admin components"
  type        = string
  default     = "econ-graph-admin"
}

variable "admin_service_name" {
  description = "Name of the admin frontend service"
  type        = string
  default     = "admin-frontend-service"
}

variable "admin_service_port" {
  description = "Port of the admin frontend service"
  type        = number
  default     = 3000
}

variable "allowed_admin_ips" {
  description = "List of IP ranges allowed to access admin interface"
  type        = list(string)

  validation {
    condition     = length(var.allowed_admin_ips) > 0
    error_message = "At least one IP range must be specified for admin access."
  }
}

variable "admin_tls_cert" {
  description = "TLS certificate for admin domain (base64 encoded)"
  type        = string
  sensitive   = true
}

variable "admin_tls_key" {
  description = "TLS private key for admin domain (base64 encoded)"
  type        = string
  sensitive   = true
}

variable "enable_monitoring" {
  description = "Enable monitoring and alerting for admin interface"
  type        = bool
  default     = true
}

variable "rate_limit_rps" {
  description = "Rate limit in requests per second for admin interface"
  type        = number
  default     = 10

  validation {
    condition     = var.rate_limit_rps >= 1 && var.rate_limit_rps <= 100
    error_message = "Rate limit must be between 1 and 100 requests per second."
  }
}

variable "rate_limit_connections" {
  description = "Maximum concurrent connections per IP"
  type        = number
  default     = 5

  validation {
    condition     = var.rate_limit_connections >= 1 && var.rate_limit_connections <= 50
    error_message = "Connection limit must be between 1 and 50."
  }
}

variable "client_body_timeout" {
  description = "Client body timeout in seconds"
  type        = number
  default     = 10
}

variable "client_header_timeout" {
  description = "Client header timeout in seconds"
  type        = number
  default     = 10
}

variable "client_max_body_size" {
  description = "Maximum client request body size"
  type        = string
  default     = "1m"
}

variable "enable_external_auth" {
  description = "Enable external authentication service"
  type        = bool
  default     = false
}

variable "auth_service_url" {
  description = "URL of external authentication service"
  type        = string
  default     = ""
}

variable "auth_signin_url" {
  description = "URL for authentication sign-in redirect"
  type        = string
  default     = ""
}

variable "security_headers" {
  description = "Additional security headers to add"
  type        = map(string)
  default = {
    "X-Frame-Options"           = "DENY"
    "X-Content-Type-Options"    = "nosniff"
    "X-XSS-Protection"          = "1; mode=block"
    "Referrer-Policy"           = "no-referrer"
    "Permissions-Policy"        = "geolocation=(), microphone=(), camera=()"
  }
}

variable "blocked_paths" {
  description = "List of path patterns to block (regex)"
  type        = list(string)
  default = [
    "\\.(php|asp|aspx|jsp)$",
    "/\\.",
    "/(wp-|wordpress|admin|phpmyadmin)",
    "/\\.git",
    "/\\.env",
    "/config\\.",
    "/backup"
  ]
}

variable "alert_email" {
  description = "Email address for security alerts"
  type        = string
  default     = ""
}

variable "alert_webhook" {
  description = "Webhook URL for security alerts"
  type        = string
  default     = ""
}

variable "log_retention_days" {
  description = "Number of days to retain access logs"
  type        = number
  default     = 90

  validation {
    condition     = var.log_retention_days >= 30 && var.log_retention_days <= 365
    error_message = "Log retention must be between 30 and 365 days."
  }
}

variable "enable_geo_blocking" {
  description = "Enable geographic IP blocking"
  type        = bool
  default     = false
}

variable "blocked_countries" {
  description = "List of country codes to block (ISO 3166-1 alpha-2)"
  type        = list(string)
  default     = []
}

variable "allowed_countries" {
  description = "List of country codes to allow (empty = allow all)"
  type        = list(string)
  default     = []
}

variable "enable_waf" {
  description = "Enable Web Application Firewall rules"
  type        = bool
  default     = true
}

variable "waf_rules" {
  description = "Custom WAF rules to apply"
  type        = list(object({
    name        = string
    priority    = number
    action      = string
    condition   = string
    description = string
  }))
  default = [
    {
      name        = "block_sql_injection"
      priority    = 100
      action      = "block"
      condition   = "contains(lower(request.uri), 'union select')"
      description = "Block SQL injection attempts"
    },
    {
      name        = "block_xss"
      priority    = 200
      action      = "block"
      condition   = "contains(lower(request.uri), '<script')"
      description = "Block XSS attempts"
    }
  ]
}

variable "maintenance_mode" {
  description = "Enable maintenance mode (returns 503 for all requests)"
  type        = bool
  default     = false
}

variable "maintenance_allowed_ips" {
  description = "IP addresses allowed during maintenance mode"
  type        = list(string)
  default     = []
}

variable "ingress_class" {
  description = "Ingress class name for admin interface"
  type        = string
  default     = "nginx-admin"
}

variable "load_balancer_source_ranges" {
  description = "Source IP ranges allowed by load balancer"
  type        = list(string)
  default     = []
}
