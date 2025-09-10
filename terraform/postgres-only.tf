# Minimal Terraform configuration to deploy only PostgreSQL
terraform {
  required_version = ">= 1.0"
  required_providers {
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "~> 2.23"
    }
    random = {
      source  = "hashicorp/random"
      version = "~> 3.4"
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
  description = "PostgreSQL database password"
  type        = string
  default     = "password"
}

# Generate random password for internal services
resource "random_password" "database_password" {
  length  = 32
  special = true
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
  password  = var.database_password != "" ? var.database_password : random_password.database_password.result

  depends_on = [kubernetes_namespace.econgraph]
}

# Outputs
output "database_password" {
  description = "PostgreSQL database password"
  value       = var.database_password != "" ? var.database_password : random_password.database_password.result
  sensitive   = true
}

output "connection_string" {
  description = "PostgreSQL connection string"
  value       = module.postgresql.connection_string
  sensitive   = true
}
