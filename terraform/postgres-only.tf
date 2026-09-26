# Minimal Terraform configuration to deploy only PostgreSQL
terraform {
  required_version = ">= 1.0"
  required_providers {
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "~> 2.23"
    }
  }
}

# Configure providers
provider "kubernetes" {
  config_path = "~/.kube/config"
}

# Variables
variable "namespace" {
  description = "Kubernetes namespace for EconGraph"
  type        = string
  default     = "econ-graph"
}

variable "database_password" {
  description = "PostgreSQL database password (required; set via TF_VAR_database_password or an untracked tfvars file)"
  type        = string
  sensitive   = true

  validation {
    condition     = length(trimspace(var.database_password)) >= 16 && !contains(["password", "changeme"], lower(trimspace(var.database_password)))
    error_message = "database_password must be a real secret of at least 16 characters, not a placeholder such as \"password\" or \"changeme\"."
  }
}

# Create namespace
resource "kubernetes_namespace" "econgraph" {
  metadata {
    name = var.namespace
    labels = {
      "app.kubernetes.io/name"    = "econgraph"
      "app.kubernetes.io/version" = "1.0.0"
    }
  }
}

# PostgreSQL StatefulSet
module "postgresql" {
  source = "./modules/postgresql"

  namespace = kubernetes_namespace.econgraph.metadata[0].name
  password  = var.database_password

  depends_on = [kubernetes_namespace.econgraph]
}

# Outputs
output "database_password" {
  description = "PostgreSQL database password"
  value       = var.database_password
  sensitive   = true
}

output "connection_string" {
  description = "PostgreSQL connection string"
  value       = module.postgresql.connection_string
  sensitive   = true
}
