# REQUIREMENT: Variables for secure admin frontend deployment
# PURPOSE: Configure admin UI deployment with security parameters
# This ensures proper configuration of the isolated administrative interface

variable "image_repository" {
  description = "Container registry repository for admin images"
  type        = string
  default     = "your-registry.com/econ-graph"
}

variable "image_tag" {
  description = "Image tag for admin frontend"
  type        = string
  default     = "latest"
}

variable "replica_count" {
  description = "Number of admin frontend replicas"
  type        = number
  default     = 2

  validation {
    condition     = var.replica_count >= 1 && var.replica_count <= 10
    error_message = "Replica count must be between 1 and 10."
  }
}

variable "min_replicas" {
  description = "Minimum number of replicas for HPA"
  type        = number
  default     = 1
}

variable "max_replicas" {
  description = "Maximum number of replicas for HPA"
  type        = number
  default     = 5
}

variable "cpu_request" {
  description = "CPU request for admin frontend pods"
  type        = string
  default     = "100m"
}

variable "cpu_limit" {
  description = "CPU limit for admin frontend pods"
  type        = string
  default     = "500m"
}

variable "memory_request" {
  description = "Memory request for admin frontend pods"
  type        = string
  default     = "128Mi"
}

variable "memory_limit" {
  description = "Memory limit for admin frontend pods"
  type        = string
  default     = "512Mi"
}

# Security configuration
variable "admin_jwt_secret" {
  description = "JWT secret for admin authentication"
  type        = string
  sensitive   = true
}

variable "admin_session_key" {
  description = "Session encryption key for admin interface"
  type        = string
  sensitive   = true
}

variable "admin_encryption_key" {
  description = "Encryption key for sensitive admin data"
  type        = string
  sensitive   = true
}

variable "allowed_admin_ips" {
  description = "List of IP ranges allowed to access admin interface"
  type        = list(string)
  default     = ["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"]

  validation {
    condition = length(var.allowed_admin_ips) > 0
    error_message = "At least one IP range must be specified for admin access."
  }
}

variable "session_timeout" {
  description = "Admin session timeout in seconds"
  type        = number
  default     = 1800  # 30 minutes

  validation {
    condition     = var.session_timeout >= 300 && var.session_timeout <= 7200
    error_message = "Session timeout must be between 300 (5 minutes) and 7200 (2 hours) seconds."
  }
}

variable "mfa_required" {
  description = "Whether MFA is required for admin access"
  type        = bool
  default     = true
}

variable "audit_logging_enabled" {
  description = "Whether audit logging is enabled"
  type        = bool
  default     = true
}

# Network configuration
variable "admin_port" {
  description = "Port for admin interface"
  type        = number
  default     = 3001

  validation {
    condition     = var.admin_port >= 1024 && var.admin_port <= 65535
    error_message = "Admin port must be between 1024 and 65535."
  }
}

variable "backend_admin_port" {
  description = "Backend admin API port"
  type        = number
  default     = 8081
}

# Monitoring and alerting
variable "enable_monitoring" {
  description = "Enable monitoring for admin interface"
  type        = bool
  default     = true
}

variable "alert_email" {
  description = "Email address for security alerts"
  type        = string
  default     = ""
}

# Environment configuration
variable "environment" {
  description = "Deployment environment"
  type        = string
  default     = "production"

  validation {
    condition     = contains(["development", "staging", "production"], var.environment)
    error_message = "Environment must be one of: development, staging, production."
  }
}

variable "cluster_name" {
  description = "Kubernetes cluster name"
  type        = string
}

variable "namespace_name" {
  description = "Kubernetes namespace for admin components"
  type        = string
  default     = "econ-graph-admin"
}
