# REQUIREMENT: PostgreSQL database deployment for time series data storage
# PURPOSE: Scalable database deployment with persistence and backup
# This provides a production-ready PostgreSQL instance for economic data

variable "namespace" {
  description = "Kubernetes namespace"
  type        = string
}

variable "password" {
  description = "PostgreSQL password"
  type        = string
  sensitive   = true
}

variable "storage_class" {
  description = "Storage class for persistent volumes"
  type        = string
  default     = "standard"
}

variable "storage_size" {
  description = "Storage size for PostgreSQL"
  type        = string
  default     = "50Gi"
}

# PostgreSQL ConfigMap
resource "kubernetes_config_map" "postgresql_config" {
  metadata {
    name      = "postgresql-config"
    namespace = var.namespace
  }

  data = {
    "postgresql.conf" = <<-EOT
      # PostgreSQL configuration optimized for time series data

      # Memory settings
      shared_buffers = 256MB
      effective_cache_size = 1GB
      work_mem = 16MB
      maintenance_work_mem = 256MB

      # Checkpoint settings
      checkpoint_completion_target = 0.9
      wal_buffers = 16MB

      # Query planner
      random_page_cost = 1.1
      effective_io_concurrency = 200

      # Logging
      log_destination = 'stderr'
      logging_collector = on
      log_directory = 'pg_log'
      log_filename = 'postgresql-%Y-%m-%d_%H%M%S.log'
      log_statement = 'all'
      log_min_duration_statement = 1000

      # Connection settings
      max_connections = 100

      # Time series optimizations
      max_wal_size = 2GB
      min_wal_size = 1GB
    EOT

    "init.sql" = <<-EOT
      -- Initialize database for EconGraph
      CREATE DATABASE econ_graph;
      CREATE USER econgraph WITH ENCRYPTED PASSWORD '${var.password}';
      GRANT ALL PRIVILEGES ON DATABASE econ_graph TO econgraph;

      -- Connect to the database
      \c econ_graph;

      -- Enable required extensions
      CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
      CREATE EXTENSION IF NOT EXISTS "pg_stat_statements";
      CREATE EXTENSION IF NOT EXISTS "pg_trgm";

      -- Grant permissions on extensions
      GRANT ALL ON SCHEMA public TO econgraph;
    EOT
  }
}

# PostgreSQL Secret
resource "kubernetes_secret" "postgresql_secret" {
  metadata {
    name      = "postgresql-secret"
    namespace = var.namespace
  }

  data = {
    "POSTGRES_PASSWORD" = base64encode(var.password)
    "POSTGRES_USER"     = base64encode("econgraph")
    "POSTGRES_DB"       = base64encode("econ_graph")
  }
}

# PostgreSQL StatefulSet
resource "kubernetes_stateful_set" "postgresql" {
  metadata {
    name      = "postgresql"
    namespace = var.namespace
    labels = {
      app     = "postgresql"
      version = "18"
    }
  }

  spec {
    service_name = "postgresql"
    replicas     = 1

    selector {
      match_labels = {
        app = "postgresql"
      }
    }

    template {
      metadata {
        labels = {
          app     = "postgresql"
          version = "18"
        }
      }

      spec {
        container {
          name  = "postgresql"
          image = "postgres:18-alpine"

          port {
            container_port = 5432
            name          = "postgresql"
          }

          env_from {
            secret_ref {
              name = kubernetes_secret.postgresql_secret.metadata[0].name
            }
          }

          env {
            name  = "PGDATA"
            value = "/var/lib/postgresql/data/pgdata"
          }

          volume_mount {
            name       = "postgresql-data"
            mount_path = "/var/lib/postgresql/data"
          }

          volume_mount {
            name       = "postgresql-config"
            mount_path = "/etc/postgresql"
          }

          volume_mount {
            name       = "postgresql-init"
            mount_path = "/docker-entrypoint-initdb.d"
          }

          # Resource limits
          resources {
            requests = {
              memory = "512Mi"
              cpu    = "250m"
            }
            limits = {
              memory = "2Gi"
              cpu    = "1000m"
            }
          }

          # Health checks
          liveness_probe {
            exec {
              command = ["pg_isready", "-U", "econgraph", "-d", "econ_graph"]
            }
            initial_delay_seconds = 30
            period_seconds        = 10
            timeout_seconds       = 5
            failure_threshold     = 3
          }

          readiness_probe {
            exec {
              command = ["pg_isready", "-U", "econgraph", "-d", "econ_graph"]
            }
            initial_delay_seconds = 5
            period_seconds        = 5
            timeout_seconds       = 3
            failure_threshold     = 3
          }
        }

        volume {
          name = "postgresql-config"
          config_map {
            name = kubernetes_config_map.postgresql_config.metadata[0].name
            items {
              key  = "postgresql.conf"
              path = "postgresql.conf"
            }
          }
        }

        volume {
          name = "postgresql-init"
          config_map {
            name = kubernetes_config_map.postgresql_config.metadata[0].name
            items {
              key  = "init.sql"
              path = "init.sql"
            }
          }
        }

        # Security context
        security_context {
          fs_group = 999
        }
      }
    }

    volume_claim_template {
      metadata {
        name = "postgresql-data"
      }
      spec {
        access_modes       = ["ReadWriteOnce"]
        storage_class_name = var.storage_class
        resources {
          requests = {
            storage = var.storage_size
          }
        }
      }
    }
  }
}

# PostgreSQL Service
resource "kubernetes_service" "postgresql" {
  metadata {
    name      = "postgresql"
    namespace = var.namespace
    labels = {
      app = "postgresql"
    }
  }

  spec {
    selector = {
      app = "postgresql"
    }

    port {
      port        = 5432
      target_port = 5432
      name        = "postgresql"
    }

    type = "ClusterIP"
  }
}

# PostgreSQL Headless Service for StatefulSet
resource "kubernetes_service" "postgresql_headless" {
  metadata {
    name      = "postgresql-headless"
    namespace = var.namespace
    labels = {
      app = "postgresql"
    }
  }

  spec {
    selector = {
      app = "postgresql"
    }

    port {
      port        = 5432
      target_port = 5432
      name        = "postgresql"
    }

    cluster_ip = "None"
    type       = "ClusterIP"
  }
}

# PostgreSQL ServiceMonitor for Prometheus
resource "kubernetes_manifest" "postgresql_servicemonitor" {
  manifest = {
    apiVersion = "monitoring.coreos.com/v1"
    kind       = "ServiceMonitor"
    metadata = {
      name      = "postgresql"
      namespace = var.namespace
      labels = {
        app = "postgresql"
      }
    }
    spec = {
      selector = {
        matchLabels = {
          app = "postgresql"
        }
      }
      endpoints = [
        {
          port     = "postgresql"
          interval = "30s"
          path     = "/metrics"
        }
      ]
    }
  }
}

# Outputs
output "service_name" {
  description = "PostgreSQL service name"
  value       = kubernetes_service.postgresql.metadata[0].name
}

output "service_port" {
  description = "PostgreSQL service port"
  value       = kubernetes_service.postgresql.spec[0].port[0].port
}

output "connection_string" {
  description = "PostgreSQL connection string"
  value       = "postgresql://econgraph:${var.password}@${kubernetes_service.postgresql.metadata[0].name}.${var.namespace}.svc.cluster.local:5432/econ_graph"
  sensitive   = true
}
