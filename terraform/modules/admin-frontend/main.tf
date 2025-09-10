# REQUIREMENT: Secure administrative interface with network isolation
# PURPOSE: Deploy admin UI on separate port with restricted access
# This ensures administrative functions are isolated from public traffic

terraform {
  required_providers {
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "~> 2.23"
    }
  }
}

# Namespace for admin components
resource "kubernetes_namespace" "admin" {
  metadata {
    name = "econ-graph-admin"

    labels = {
      name                 = "econ-graph-admin"
      "app.kubernetes.io/component" = "admin"
      "security.level"     = "restricted"
    }

    annotations = {
      "description" = "Administrative interface - RESTRICTED ACCESS"
    }
  }
}

# Network policy to isolate admin namespace
resource "kubernetes_network_policy" "admin_isolation" {
  metadata {
    name      = "admin-network-isolation"
    namespace = kubernetes_namespace.admin.metadata[0].name
  }

  spec {
    pod_selector {}

    policy_types = ["Ingress", "Egress"]

    # Only allow ingress from specific sources
    ingress {
      from {
        # Allow from monitoring namespace
        namespace_selector {
          match_labels = {
            name = "monitoring"
          }
        }
      }

      from {
        # Allow from specific admin IP ranges (customize as needed)
        ip_block {
          cidr = "10.0.0.0/8"  # Internal cluster traffic
        }
      }

      ports {
        protocol = "TCP"
        port     = "3000"
      }
    }

    # Restrict egress
    egress {
      # Allow DNS resolution
      ports {
        protocol = "UDP"
        port     = "53"
      }
    }

    egress {
      # Allow HTTPS to backend
      to {
        namespace_selector {
          match_labels = {
            name = "econ-graph"
          }
        }
      }

      ports {
        protocol = "TCP"
        port     = "8081"  # Admin backend port
      }
    }
  }
}

# ConfigMap for admin frontend configuration
resource "kubernetes_config_map" "admin_config" {
  metadata {
    name      = "admin-frontend-config"
    namespace = kubernetes_namespace.admin.metadata[0].name
  }

  data = {
    REACT_APP_ENV                = "admin"
    REACT_APP_API_URL           = "http://backend-admin-service:8081"
    REACT_APP_ENABLE_SECURITY   = "true"
    REACT_APP_SESSION_TIMEOUT   = "1800"  # 30 minutes
    REACT_APP_MFA_REQUIRED      = "true"
    REACT_APP_AUDIT_LOGGING     = "true"
  }
}

# Secret for admin authentication
resource "kubernetes_secret" "admin_auth" {
  metadata {
    name      = "admin-auth-secret"
    namespace = kubernetes_namespace.admin.metadata[0].name

    annotations = {
      "kubernetes.io/service-account.name" = "admin-frontend"
    }
  }

  data = {
    jwt-secret     = var.admin_jwt_secret
    session-key    = var.admin_session_key
    encryption-key = var.admin_encryption_key
  }

  type = "Opaque"
}

# Admin frontend deployment
resource "kubernetes_deployment" "admin_frontend" {
  metadata {
    name      = "admin-frontend"
    namespace = kubernetes_namespace.admin.metadata[0].name

    labels = {
      app                          = "admin-frontend"
      version                      = var.image_tag
      "app.kubernetes.io/name"     = "econ-graph-admin"
      "app.kubernetes.io/component" = "frontend"
    }
  }

  spec {
    replicas = var.replica_count

    selector {
      match_labels = {
        app = "admin-frontend"
      }
    }

    template {
      metadata {
        labels = {
          app = "admin-frontend"
        }

        annotations = {
          "security.level" = "restricted"
          "access.policy"  = "admin-only"
        }
      }

      spec {
        service_account_name = kubernetes_service_account.admin_frontend.metadata[0].name

        # Security context
        security_context {
          run_as_non_root = true
          run_as_user     = 1000
          fs_group        = 1000

          seccomp_profile {
            type = "RuntimeDefault"
          }
        }

        container {
          name  = "admin-frontend"
          image = "${var.image_repository}/econ-graph-admin:${var.image_tag}"

          port {
            container_port = 3000
            name          = "http"
            protocol      = "TCP"
          }

          env_from {
            config_map_ref {
              name = kubernetes_config_map.admin_config.metadata[0].name
            }
          }

          env {
            name = "NODE_ENV"
            value = "production"
          }

          env {
            name = "REACT_APP_BUILD_TIME"
            value = timestamp()
          }

          # Security context for container
          security_context {
            allow_privilege_escalation = false
            read_only_root_filesystem  = true
            run_as_non_root           = true
            run_as_user               = 1000

            capabilities {
              drop = ["ALL"]
            }
          }

          # Resource limits
          resources {
            limits = {
              cpu    = var.cpu_limit
              memory = var.memory_limit
            }
            requests = {
              cpu    = var.cpu_request
              memory = var.memory_request
            }
          }

          # Health checks
          liveness_probe {
            http_get {
              path = "/health"
              port = 3000
            }
            initial_delay_seconds = 30
            period_seconds        = 10
            failure_threshold     = 3
          }

          readiness_probe {
            http_get {
              path = "/ready"
              port = 3000
            }
            initial_delay_seconds = 5
            period_seconds        = 5
            failure_threshold     = 3
          }

          # Volume mounts for writable directories
          volume_mount {
            name       = "tmp"
            mount_path = "/tmp"
          }

          volume_mount {
            name       = "cache"
            mount_path = "/app/.cache"
          }
        }

        # Volumes
        volume {
          name = "tmp"
          empty_dir {}
        }

        volume {
          name = "cache"
          empty_dir {}
        }

        # Pod anti-affinity for high availability
        affinity {
          pod_anti_affinity {
            preferred_during_scheduling_ignored_during_execution {
              weight = 100
              pod_affinity_term {
                label_selector {
                  match_expressions {
                    key      = "app"
                    operator = "In"
                    values   = ["admin-frontend"]
                  }
                }
                topology_key = "kubernetes.io/hostname"
              }
            }
          }
        }
      }
    }

    strategy {
      type = "RollingUpdate"
      rolling_update {
        max_unavailable = "25%"
        max_surge       = "25%"
      }
    }
  }
}

# Service account for admin frontend
resource "kubernetes_service_account" "admin_frontend" {
  metadata {
    name      = "admin-frontend"
    namespace = kubernetes_namespace.admin.metadata[0].name

    annotations = {
      "description" = "Service account for admin frontend"
    }
  }
}

# ClusterRole for admin frontend (minimal permissions)
resource "kubernetes_cluster_role" "admin_frontend" {
  metadata {
    name = "admin-frontend-role"
  }

  rule {
    api_groups = [""]
    resources  = ["configmaps", "secrets"]
    verbs      = ["get", "list"]
    resource_names = [
      kubernetes_config_map.admin_config.metadata[0].name,
      kubernetes_secret.admin_auth.metadata[0].name
    ]
  }
}

# ClusterRoleBinding for admin frontend
resource "kubernetes_cluster_role_binding" "admin_frontend" {
  metadata {
    name = "admin-frontend-binding"
  }

  role_ref {
    api_group = "rbac.authorization.k8s.io"
    kind      = "ClusterRole"
    name      = kubernetes_cluster_role.admin_frontend.metadata[0].name
  }

  subject {
    kind      = "ServiceAccount"
    name      = kubernetes_service_account.admin_frontend.metadata[0].name
    namespace = kubernetes_namespace.admin.metadata[0].name
  }
}

# Internal service (not exposed externally)
resource "kubernetes_service" "admin_frontend" {
  metadata {
    name      = "admin-frontend-service"
    namespace = kubernetes_namespace.admin.metadata[0].name

    labels = {
      app = "admin-frontend"
    }

    annotations = {
      "description" = "Admin frontend internal service"
      "access.policy" = "internal-only"
    }
  }

  spec {
    selector = {
      app = "admin-frontend"
    }

    port {
      name        = "http"
      port        = 3000
      target_port = 3000
      protocol    = "TCP"
    }

    type                    = "ClusterIP"
    internal_traffic_policy = "Local"
  }
}

# Pod Disruption Budget
resource "kubernetes_pod_disruption_budget" "admin_frontend" {
  metadata {
    name      = "admin-frontend-pdb"
    namespace = kubernetes_namespace.admin.metadata[0].name
  }

  spec {
    min_available = "50%"

    selector {
      match_labels = {
        app = "admin-frontend"
      }
    }
  }
}

# Horizontal Pod Autoscaler
resource "kubernetes_horizontal_pod_autoscaler_v2" "admin_frontend" {
  metadata {
    name      = "admin-frontend-hpa"
    namespace = kubernetes_namespace.admin.metadata[0].name
  }

  spec {
    scale_target_ref {
      api_version = "apps/v1"
      kind        = "Deployment"
      name        = kubernetes_deployment.admin_frontend.metadata[0].name
    }

    min_replicas = var.min_replicas
    max_replicas = var.max_replicas

    metric {
      type = "Resource"
      resource {
        name = "cpu"
        target {
          type                = "Utilization"
          average_utilization = 70
        }
      }
    }

    metric {
      type = "Resource"
      resource {
        name = "memory"
        target {
          type                = "Utilization"
          average_utilization = 80
        }
      }
    }
  }
}
