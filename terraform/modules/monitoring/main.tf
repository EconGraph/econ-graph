# REQUIREMENT: Grafana dashboards for monitoring usage of the backend, database statistics, and crawl status
# PURPOSE: Complete monitoring stack with Prometheus and Grafana for system observability
# This provides comprehensive monitoring and alerting for the EconGraph platform

variable "namespace" {
  description = "Kubernetes namespace"
  type        = string
}

variable "grafana_admin_password" {
  description = "Grafana admin password"
  type        = string
  sensitive   = true
}

variable "domain" {
  description = "Domain name for the application"
  type        = string
}

variable "prometheus_retention" {
  description = "Prometheus data retention period"
  type        = string
  default     = "30d"
}

variable "prometheus_storage_size" {
  description = "Prometheus storage size"
  type        = string
  default     = "50Gi"
}

# Install Prometheus using Helm
resource "helm_release" "prometheus" {
  name       = "prometheus"
  repository = "https://prometheus-community.github.io/helm-charts"
  chart      = "kube-prometheus-stack"
  version    = "54.0.0"
  namespace  = var.namespace

  values = [
    yamlencode({
      prometheus = {
        prometheusSpec = {
          retention    = var.prometheus_retention
          storageSpec = {
            volumeClaimTemplate = {
              spec = {
                storageClassName = "standard"
                accessModes      = ["ReadWriteOnce"]
                resources = {
                  requests = {
                    storage = var.prometheus_storage_size
                  }
                }
              }
            }
          }
          additionalScrapeConfigs = [
            {
              job_name        = "econgraph-backend"
              static_configs = [
                {
                  targets = ["econgraph-backend.${var.namespace}.svc.cluster.local:9090"]
                }
              ]
              metrics_path = "/metrics"
            },
            {
              job_name        = "econgraph-crawler"
              static_configs = [
                {
                  targets = ["econgraph-crawler.${var.namespace}.svc.cluster.local:9090"]
                }
              ]
              metrics_path = "/metrics"
            }
          ]
        }
      }

      grafana = {
        adminPassword = var.grafana_admin_password
        ingress = {
          enabled = true
          hosts   = ["grafana.${var.domain}"]
          annotations = {
            "kubernetes.io/ingress.class"                = "nginx"
            "cert-manager.io/cluster-issuer"             = "letsencrypt-prod"
            "nginx.ingress.kubernetes.io/rewrite-target" = "/"
          }
          tls = [
            {
              secretName = "grafana-tls"
              hosts      = ["grafana.${var.domain}"]
            }
          ]
        }

        # Grafana configuration
        grafana.ini = {
          server = {
            root_url = "https://grafana.${var.domain}"
          }
          security = {
            admin_password = var.grafana_admin_password
          }
          "auth.anonymous" = {
            enabled = false
          }
        }

        # Data sources
        datasources = {
          "datasources.yaml" = {
            apiVersion = 1
            datasources = [
              {
                name      = "Prometheus"
                type      = "prometheus"
                url       = "http://prometheus-kube-prometheus-prometheus:9090"
                access    = "proxy"
                isDefault = true
              }
            ]
          }
        }

        # Dashboard providers
        dashboardProviders = {
          "dashboardproviders.yaml" = {
            apiVersion = 1
            providers = [
              {
                name            = "default"
                orgId           = 1
                folder          = ""
                type            = "file"
                disableDeletion = false
                editable        = true
                options = {
                  path = "/var/lib/grafana/dashboards/default"
                }
              }
            ]
          }
        }

        # Custom dashboards
        dashboards = {
          default = {
            "econgraph-overview" = {
              gnetId    = null
              revision  = null
              datasource = "Prometheus"
              json = jsonencode({
                dashboard = {
                  id    = null
                  title = "EconGraph Overview"
                  tags  = ["econgraph"]
                  timezone = "browser"
                  panels = [
                    {
                      id    = 1
                      title = "Backend Response Time"
                      type  = "graph"
                      targets = [
                        {
                          expr         = "histogram_quantile(0.95, sum(rate(http_request_duration_seconds_bucket{job=\"econgraph-backend\"}[5m])) by (le))"
                          legendFormat = "95th percentile"
                        }
                      ]
                      gridPos = {
                        h = 8
                        w = 12
                        x = 0
                        y = 0
                      }
                    },
                    {
                      id    = 2
                      title = "Request Rate"
                      type  = "graph"
                      targets = [
                        {
                          expr         = "sum(rate(http_requests_total{job=\"econgraph-backend\"}[5m]))"
                          legendFormat = "Requests/sec"
                        }
                      ]
                      gridPos = {
                        h = 8
                        w = 12
                        x = 12
                        y = 0
                      }
                    }
                  ]
                  time = {
                    from = "now-1h"
                    to   = "now"
                  }
                  refresh = "30s"
                }
              })
            }
          }
        }
      }

      # Alertmanager configuration
      alertmanager = {
        alertmanagerSpec = {
          storage = {
            volumeClaimTemplate = {
              spec = {
                storageClassName = "standard"
                accessModes      = ["ReadWriteOnce"]
                resources = {
                  requests = {
                    storage = "10Gi"
                  }
                }
              }
            }
          }
        }
      }
    })
  ]

  depends_on = [kubernetes_namespace.monitoring]
}

# Create monitoring namespace if it doesn't exist
resource "kubernetes_namespace" "monitoring" {
  metadata {
    name = "${var.namespace}-monitoring"
    labels = {
      "app.kubernetes.io/name" = "monitoring"
    }
  }
}

# Custom Grafana dashboard ConfigMaps
resource "kubernetes_config_map" "econgraph_dashboard" {
  metadata {
    name      = "econgraph-dashboard"
    namespace = var.namespace
    labels = {
      grafana_dashboard = "1"
    }
  }

  data = {
    "econgraph-system.json" = jsonencode({
      dashboard = {
        id          = null
        uid         = "econgraph-system"
        title       = "EconGraph System Metrics"
        description = "System-level metrics for EconGraph platform"
        tags        = ["econgraph", "system"]
        timezone    = "browser"
        schemaVersion = 27
        version     = 1

        panels = [
          # Backend metrics
          {
            id    = 1
            title = "Backend Pods Status"
            type  = "stat"
            targets = [
              {
                expr         = "sum(up{job=\"econgraph-backend\"})"
                legendFormat = "Running Pods"
              }
            ]
            gridPos = { h = 4, w = 6, x = 0, y = 0 }
            fieldConfig = {
              defaults = {
                color = { mode = "thresholds" }
                thresholds = {
                  steps = [
                    { color = "red", value = 0 },
                    { color = "yellow", value = 1 },
                    { color = "green", value = 2 }
                  ]
                }
              }
            }
          },

          # Database connections
          {
            id    = 2
            title = "Database Connections"
            type  = "graph"
            targets = [
              {
                expr         = "sum(pg_stat_activity_count{datname=\"econ_graph\"})"
                legendFormat = "Active Connections"
              }
            ]
            gridPos = { h = 8, w = 12, x = 0, y = 4 }
          },

          # Crawler queue status
          {
            id    = 3
            title = "Crawler Queue Status"
            type  = "graph"
            targets = [
              {
                expr         = "sum(econgraph_queue_items{status=\"pending\"})"
                legendFormat = "Pending Items"
              },
              {
                expr         = "sum(econgraph_queue_items{status=\"processing\"})"
                legendFormat = "Processing Items"
              },
              {
                expr         = "sum(econgraph_queue_items{status=\"completed\"})"
                legendFormat = "Completed Items"
              }
            ]
            gridPos = { h = 8, w = 12, x = 12, y = 4 }
          },

          # API Response times
          {
            id    = 4
            title = "GraphQL Response Times"
            type  = "graph"
            targets = [
              {
                expr         = "histogram_quantile(0.50, sum(rate(graphql_request_duration_seconds_bucket[5m])) by (le))"
                legendFormat = "50th percentile"
              },
              {
                expr         = "histogram_quantile(0.95, sum(rate(graphql_request_duration_seconds_bucket[5m])) by (le))"
                legendFormat = "95th percentile"
              },
              {
                expr         = "histogram_quantile(0.99, sum(rate(graphql_request_duration_seconds_bucket[5m])) by (le))"
                legendFormat = "99th percentile"
              }
            ]
            gridPos = { h = 8, w = 24, x = 0, y = 12 }
          }
        ]

        time = {
          from = "now-1h"
          to   = "now"
        }
        refresh = "30s"
      }
    })
  }
}

# Database monitoring dashboard
resource "kubernetes_config_map" "database_dashboard" {
  metadata {
    name      = "database-dashboard"
    namespace = var.namespace
    labels = {
      grafana_dashboard = "1"
    }
  }

  data = {
    "econgraph-database.json" = jsonencode({
      dashboard = {
        id          = null
        uid         = "econgraph-database"
        title       = "EconGraph Database Statistics"
        description = "PostgreSQL database metrics and statistics"
        tags        = ["econgraph", "database", "postgresql"]
        timezone    = "browser"
        schemaVersion = 27
        version     = 1

        panels = [
          # Database size
          {
            id    = 1
            title = "Database Size"
            type  = "stat"
            targets = [
              {
                expr         = "pg_database_size_bytes{datname=\"econ_graph\"}"
                legendFormat = "Database Size (Bytes)"
              }
            ]
            gridPos = { h = 4, w = 6, x = 0, y = 0 }
            fieldConfig = {
              defaults = {
                unit = "bytes"
                color = { mode = "continuous-GrYlRd" }
              }
            }
          },

          # Table sizes
          {
            id    = 2
            title = "Table Sizes"
            type  = "table"
            targets = [
              {
                expr         = "pg_stat_user_tables_n_tup_ins + pg_stat_user_tables_n_tup_upd + pg_stat_user_tables_n_tup_del"
                legendFormat = "{{schemaname}}.{{relname}}"
              }
            ]
            gridPos = { h = 8, w = 12, x = 0, y = 4 }
          },

          # Query performance
          {
            id    = 3
            title = "Slow Queries"
            type  = "graph"
            targets = [
              {
                expr         = "rate(pg_stat_statements_mean_time_seconds[5m])"
                legendFormat = "Mean Query Time"
              }
            ]
            gridPos = { h = 8, w = 12, x = 12, y = 4 }
          },

          # Connection pool
          {
            id    = 4
            title = "Connection Pool Status"
            type  = "graph"
            targets = [
              {
                expr         = "pg_stat_activity_count"
                legendFormat = "Active Connections"
              },
              {
                expr         = "pg_settings_max_connections"
                legendFormat = "Max Connections"
              }
            ]
            gridPos = { h = 8, w = 24, x = 0, y = 12 }
          }
        ]

        time = {
          from = "now-6h"
          to   = "now"
        }
        refresh = "1m"
      }
    })
  }
}

# Crawler monitoring dashboard
resource "kubernetes_config_map" "crawler_dashboard" {
  metadata {
    name      = "crawler-dashboard"
    namespace = var.namespace
    labels = {
      grafana_dashboard = "1"
    }
  }

  data = {
    "econgraph-crawler.json" = jsonencode({
      dashboard = {
        id          = null
        uid         = "econgraph-crawler"
        title       = "EconGraph Crawler Status"
        description = "Data crawler monitoring and queue processing metrics"
        tags        = ["econgraph", "crawler", "queue"]
        timezone    = "browser"
        schemaVersion = 27
        version     = 1

        panels = [
          # Active crawlers
          {
            id    = 1
            title = "Active Crawler Instances"
            type  = "stat"
            targets = [
              {
                expr         = "sum(up{job=\"econgraph-crawler\"})"
                legendFormat = "Active Crawlers"
              }
            ]
            gridPos = { h = 4, w = 6, x = 0, y = 0 }
          },

          # Queue processing rate
          {
            id    = 2
            title = "Queue Processing Rate"
            type  = "graph"
            targets = [
              {
                expr         = "rate(econgraph_queue_items_processed_total[5m])"
                legendFormat = "Items Processed/sec"
              }
            ]
            gridPos = { h = 8, w = 12, x = 0, y = 4 }
          },

          # Data source status
          {
            id    = 3
            title = "Data Source Crawl Status"
            type  = "table"
            targets = [
              {
                expr         = "econgraph_last_crawl_timestamp"
                legendFormat = "{{source}}"
                format      = "table"
              }
            ]
            gridPos = { h = 8, w = 12, x = 12, y = 4 }
          },

          # Error rates
          {
            id    = 4
            title = "Crawl Error Rates"
            type  = "graph"
            targets = [
              {
                expr         = "rate(econgraph_crawl_errors_total[5m])"
                legendFormat = "{{source}} Errors/sec"
              }
            ]
            gridPos = { h = 8, w = 24, x = 0, y = 12 }
          }
        ]

        time = {
          from = "now-2h"
          to   = "now"
        }
        refresh = "15s"
      }
    })
  }
}

# PrometheusRule for custom alerts
resource "kubernetes_manifest" "econgraph_alerts" {
  manifest = {
    apiVersion = "monitoring.coreos.com/v1"
    kind       = "PrometheusRule"
    metadata = {
      name      = "econgraph-alerts"
      namespace = var.namespace
      labels = {
        app = "econgraph"
      }
    }
    spec = {
      groups = [
        {
          name = "econgraph.rules"
          rules = [
            {
              alert = "EconGraphBackendDown"
              expr  = "up{job=\"econgraph-backend\"} == 0"
              for   = "5m"
              labels = {
                severity = "critical"
              }
              annotations = {
                summary     = "EconGraph backend is down"
                description = "EconGraph backend has been down for more than 5 minutes."
              }
            },
            {
              alert = "EconGraphHighResponseTime"
              expr  = "histogram_quantile(0.95, sum(rate(http_request_duration_seconds_bucket{job=\"econgraph-backend\"}[5m])) by (le)) > 1"
              for   = "10m"
              labels = {
                severity = "warning"
              }
              annotations = {
                summary     = "High response time for EconGraph backend"
                description = "95th percentile response time is above 1 second for 10 minutes."
              }
            },
            {
              alert = "EconGraphQueueBacklog"
              expr  = "sum(econgraph_queue_items{status=\"pending\"}) > 1000"
              for   = "15m"
              labels = {
                severity = "warning"
              }
              annotations = {
                summary     = "EconGraph crawler queue backlog"
                description = "More than 1000 items pending in the crawler queue for 15 minutes."
              }
            },
            {
              alert = "EconGraphDatabaseConnectionsHigh"
              expr  = "sum(pg_stat_activity_count{datname=\"econ_graph\"}) > 80"
              for   = "5m"
              labels = {
                severity = "warning"
              }
              annotations = {
                summary     = "High database connection count"
                description = "Database connection count is above 80 for 5 minutes."
              }
            }
          ]
        }
      ]
    }
  }
}

# Outputs
output "grafana_url" {
  description = "Grafana dashboard URL"
  value       = "https://grafana.${var.domain}"
}

output "prometheus_url" {
  description = "Prometheus URL (internal)"
  value       = "http://prometheus-kube-prometheus-prometheus.${var.namespace}-monitoring.svc.cluster.local:9090"
}
