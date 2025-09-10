# REQUIREMENT: Terraform deployment scripts for creating a Kubernetes cluster
# PURPOSE: Infrastructure as Code for the EconGraph platform deployment
# This creates a complete production-ready environment with monitoring and scaling

terraform {
  required_version = ">= 1.0"
  required_providers {
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "~> 2.23"
    }
    helm = {
      source  = "hashicorp/helm"
      version = "~> 2.11"
    }
    random = {
      source  = "hashicorp/random"
      version = "~> 3.4"
    }
    tls = {
      source  = "hashicorp/tls"
      version = "~> 4.0"
    }
  }
}

# Configure providers
provider "kubernetes" {
  config_path = var.kubeconfig_path
}

provider "helm" {
  kubernetes {
    config_path = var.kubeconfig_path
  }
}

# Variables
variable "kubeconfig_path" {
  description = "Path to kubeconfig file"
  type        = string
  default     = "~/.kube/config"
}

variable "namespace" {
  description = "Kubernetes namespace for EconGraph"
  type        = string
  default     = "econgraph"
}

variable "environment" {
  description = "Environment (dev, staging, prod)"
  type        = string
  default     = "prod"
}

variable "domain" {
  description = "Domain name for the application"
  type        = string
  default     = "econgraph.example.com"
}

variable "database_password" {
  description = "PostgreSQL database password"
  type        = string
  sensitive   = true
}

variable "fred_api_key" {
  description = "FRED API key for data crawling"
  type        = string
  sensitive   = true
  default     = ""
}

variable "bls_api_key" {
  description = "BLS API key for data crawling"
  type        = string
  sensitive   = true
  default     = ""
}

# Generate random passwords for internal services
resource "random_password" "database_password" {
  length  = 32
  special = true
}

resource "random_password" "grafana_admin_password" {
  length  = 16
  special = false
}

# Create namespace
resource "kubernetes_namespace" "econgraph" {
  metadata {
    name = var.namespace
    labels = {
      "app.kubernetes.io/name"    = "econgraph"
      "app.kubernetes.io/version" = "1.0.0"
      environment                 = var.environment
    }
  }
}

# Create ConfigMap for application configuration
resource "kubernetes_config_map" "econgraph_config" {
  metadata {
    name      = "econgraph-config"
    namespace = kubernetes_namespace.econgraph.metadata[0].name
  }

  data = {
    # Database configuration
    "DATABASE_HOST"     = "postgresql.${var.namespace}.svc.cluster.local"
    "DATABASE_PORT"     = "5432"
    "DATABASE_NAME"     = "econ_graph"
    "DATABASE_USER"     = "econgraph"

    # Server configuration
    "SERVER_HOST"       = "0.0.0.0"
    "SERVER_PORT"       = "8080"

    # CORS configuration
    "CORS_ALLOWED_ORIGINS" = "https://${var.domain}"

    # Logging
    "RUST_LOG"          = "info"

    # Queue processing
    "MAX_CONCURRENT_JOBS"        = "10"
    "QUEUE_POLL_INTERVAL_SECONDS" = "5"

    # Rate limiting
    "FRED_RATE_LIMIT_PER_MINUTE" = "120"
    "BLS_RATE_LIMIT_PER_MINUTE"  = "500"
  }
}

# Create Secret for sensitive configuration
resource "kubernetes_secret" "econgraph_secrets" {
  metadata {
    name      = "econgraph-secrets"
    namespace = kubernetes_namespace.econgraph.metadata[0].name
  }

  data = {
    "DATABASE_PASSWORD" = base64encode(var.database_password != "" ? var.database_password : random_password.database_password.result)
    "FRED_API_KEY"      = base64encode(var.fred_api_key)
    "BLS_API_KEY"       = base64encode(var.bls_api_key)
  }
}

# PostgreSQL StatefulSet
module "postgresql" {
  source = "./modules/postgresql"

  namespace = kubernetes_namespace.econgraph.metadata[0].name
  password  = var.database_password != "" ? var.database_password : random_password.database_password.result

  depends_on = [kubernetes_namespace.econgraph]
}

# EconGraph Backend Deployment
module "backend" {
  source = "./modules/backend"

  namespace     = kubernetes_namespace.econgraph.metadata[0].name
  environment   = var.environment
  config_map    = kubernetes_config_map.econgraph_config.metadata[0].name
  secret        = kubernetes_secret.econgraph_secrets.metadata[0].name

  depends_on = [module.postgresql]
}

# EconGraph Crawler Deployment
module "crawler" {
  source = "./modules/crawler"

  namespace     = kubernetes_namespace.econgraph.metadata[0].name
  environment   = var.environment
  config_map    = kubernetes_config_map.econgraph_config.metadata[0].name
  secret        = kubernetes_secret.econgraph_secrets.metadata[0].name

  depends_on = [module.postgresql]
}

# Frontend Deployment
module "frontend" {
  source = "./modules/frontend"

  namespace   = kubernetes_namespace.econgraph.metadata[0].name
  environment = var.environment
  domain      = var.domain

  depends_on = [module.backend]
}

# Monitoring Stack (Prometheus + Grafana)
module "monitoring" {
  source = "./modules/monitoring"

  namespace              = kubernetes_namespace.econgraph.metadata[0].name
  grafana_admin_password = random_password.grafana_admin_password.result
  domain                 = var.domain

  depends_on = [kubernetes_namespace.econgraph]
}

# Ingress Controller (if not already installed)
module "ingress" {
  source = "./modules/ingress"

  namespace = kubernetes_namespace.econgraph.metadata[0].name
  domain    = var.domain

  depends_on = [module.frontend, module.backend, module.monitoring]
}

# Outputs
output "namespace" {
  description = "Kubernetes namespace"
  value       = kubernetes_namespace.econgraph.metadata[0].name
}

output "database_password" {
  description = "PostgreSQL database password"
  value       = var.database_password != "" ? var.database_password : random_password.database_password.result
  sensitive   = true
}

output "grafana_admin_password" {
  description = "Grafana admin password"
  value       = random_password.grafana_admin_password.result
  sensitive   = true
}

output "application_urls" {
  description = "Application access URLs"
  value = {
    frontend    = "https://${var.domain}"
    backend_api = "https://api.${var.domain}"
    grafana     = "https://grafana.${var.domain}"
  }
}

# Admin Frontend (Secure, Isolated)
module "admin_frontend" {
  source = "./modules/admin-frontend"

  image_repository = var.image_repository
  image_tag        = var.image_tag
  cluster_name     = var.cluster_name

  # Security configuration
  admin_jwt_secret     = var.admin_jwt_secret
  admin_session_key    = var.admin_session_key
  admin_encryption_key = var.admin_encryption_key

  # Resource limits
  replica_count    = var.admin_replica_count
  cpu_request     = var.admin_cpu_request
  cpu_limit       = var.admin_cpu_limit
  memory_request  = var.admin_memory_request
  memory_limit    = var.admin_memory_limit

  # Network security
  allowed_admin_ips = var.allowed_admin_ips
  session_timeout   = var.admin_session_timeout
  mfa_required      = var.admin_mfa_required

  depends_on = [
    module.postgresql,
    module.backend
  ]
}

# Admin Ingress (Restricted Access)
module "admin_ingress" {
  source = "./modules/admin-ingress"

  admin_domain       = var.admin_domain
  admin_namespace    = module.admin_frontend.namespace_name
  admin_service_name = module.admin_frontend.service_name
  admin_service_port = module.admin_frontend.service_port

  # Access control
  allowed_admin_ips = var.allowed_admin_ips
  admin_tls_cert    = var.admin_tls_cert
  admin_tls_key     = var.admin_tls_key

  # Security settings
  rate_limit_rps         = var.admin_rate_limit_rps
  rate_limit_connections = var.admin_rate_limit_connections
  enable_monitoring      = var.enable_monitoring

  # Alerting
  alert_email = var.admin_alert_email

  depends_on = [
    module.admin_frontend
  ]
}

output "kubectl_commands" {
  description = "Useful kubectl commands"
  value = {
    get_pods     = "kubectl get pods -n ${var.namespace}"
    get_services = "kubectl get services -n ${var.namespace}"
    logs_backend = "kubectl logs -n ${var.namespace} -l app=econgraph-backend -f"
    logs_crawler = "kubectl logs -n ${var.namespace} -l app=econgraph-crawler -f"
    admin_pods   = "kubectl get pods -n econ-graph-admin"
    admin_logs   = "kubectl logs -n econ-graph-admin -l app=admin-frontend -f"
  }
}

output "admin_access" {
  description = "Admin interface access information"
  value = {
    admin_url     = "https://${var.admin_domain}"
    namespace     = module.admin_frontend.namespace_name
    allowed_ips   = var.allowed_admin_ips
    security_note = "Admin interface is restricted to specified IP ranges and requires authentication"
  }
  sensitive = false
}
