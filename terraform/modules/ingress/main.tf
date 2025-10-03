# REQUIREMENT: Ingress controller for external access to the application
# PURPOSE: Expose the application to the internet with SSL termination and routing
# This provides secure external access to the frontend, backend, and monitoring

variable "namespace" {
  description = "Kubernetes namespace"
  type        = string
}

variable "domain" {
  description = "Domain name for the application"
  type        = string
}

variable "enable_cert_manager" {
  description = "Enable cert-manager for automatic SSL certificates"
  type        = bool
  default     = true
}

variable "enable_dns01_challenge" {
  description = "Enable DNS-01 challenge for wildcard certificates"
  type        = bool
  default     = false
}

variable "cloudflare_api_token" {
  description = "Cloudflare API token for DNS-01 challenge"
  type        = string
  sensitive   = true
  default     = ""
}

# Install NGINX Ingress Controller
resource "helm_release" "nginx_ingress" {
  name       = "nginx-ingress"
  repository = "https://kubernetes.github.io/ingress-nginx"
  chart      = "ingress-nginx"
  version    = "4.8.0"
  namespace  = "ingress-nginx"
  create_namespace = true

  values = [
    yamlencode({
      controller = {
        replicaCount = 2

        # Resource limits
        resources = {
          limits = {
            cpu    = "1000m"
            memory = "1Gi"
          }
          requests = {
            cpu    = "100m"
            memory = "128Mi"
          }
        }

        # Metrics for monitoring
        metrics = {
          enabled = true
          serviceMonitor = {
            enabled = true
          }
        }

        # Configuration
        config = {
          "use-forwarded-headers"    = "true"
          "compute-full-forwarded-for" = "true"
          "use-proxy-protocol"       = "false"
          "server-tokens"            = "false"
          "ssl-protocols"            = "TLSv1.2 TLSv1.3"
          "ssl-ciphers"             = "ECDHE-ECDSA-AES128-GCM-SHA256,ECDHE-RSA-AES128-GCM-SHA256,ECDHE-ECDSA-AES256-GCM-SHA384,ECDHE-RSA-AES256-GCM-SHA384"
          "enable-brotli"           = "true"
          "proxy-buffer-size"       = "16k"
          "proxy-body-size"         = "50m"
        }

        # Service configuration
        service = {
          type = "LoadBalancer"
          annotations = {
            "service.beta.kubernetes.io/aws-load-balancer-type" = "nlb"
            "service.beta.kubernetes.io/aws-load-balancer-cross-zone-load-balancing-enabled" = "true"
          }
        }

        # Pod disruption budget
        podDisruptionBudget = {
          enabled      = true
          minAvailable = 1
        }

        # Auto-scaling
        autoscaling = {
          enabled     = true
          minReplicas = 2
          maxReplicas = 10
          targetCPUUtilizationPercentage = 70
        }

        # Node affinity for better distribution
        affinity = {
          podAntiAffinity = {
            preferredDuringSchedulingIgnoredDuringExecution = [
              {
                weight = 100
                podAffinityTerm = {
                  labelSelector = {
                    matchExpressions = [
                      {
                        key      = "app.kubernetes.io/name"
                        operator = "In"
                        values   = ["ingress-nginx"]
                      }
                    ]
                  }
                  topologyKey = "kubernetes.io/hostname"
                }
              }
            ]
          }
        }
      }
    })
  ]
}

# Install cert-manager for automatic SSL certificates
resource "helm_release" "cert_manager" {
  count = var.enable_cert_manager ? 1 : 0

  name       = "cert-manager"
  repository = "https://charts.jetstack.io"
  chart      = "cert-manager"
  version    = "v1.13.0"
  namespace  = "cert-manager"
  create_namespace = true

  set {
    name  = "installCRDs"
    value = "true"
  }

  values = [
    yamlencode({
      # Resource limits
      resources = {
        limits = {
          cpu    = "100m"
          memory = "128Mi"
        }
        requests = {
          cpu    = "10m"
          memory = "32Mi"
        }
      }

      webhook = {
        resources = {
          limits = {
            cpu    = "100m"
            memory = "128Mi"
          }
          requests = {
            cpu    = "10m"
            memory = "32Mi"
          }
        }
      }

      cainjector = {
        resources = {
          limits = {
            cpu    = "100m"
            memory = "128Mi"
          }
          requests = {
            cpu    = "10m"
            memory = "32Mi"
          }
        }
      }

      # Metrics for monitoring
      prometheus = {
        enabled = true
        servicemonitor = {
          enabled = true
        }
      }
    })
  ]
}

# ClusterIssuer for Let's Encrypt certificates
resource "kubernetes_manifest" "letsencrypt_prod" {
  count = var.enable_cert_manager ? 1 : 0

  manifest = {
    apiVersion = "cert-manager.io/v1"
    kind       = "ClusterIssuer"
    metadata = {
      name = "letsencrypt-prod"
    }
    spec = {
      acme = {
        server = "https://acme-v02.api.letsencrypt.org/directory"
        email  = "admin@${var.domain}"
        privateKeySecretRef = {
          name = "letsencrypt-prod"
        }
        solvers = [
          {
            http01 = {
              ingress = {
                class = "nginx"
              }
            }
          }
        ]
      }
    }
  }

  depends_on = [helm_release.cert_manager]
}

# ClusterIssuer for Let's Encrypt staging (for testing)
resource "kubernetes_manifest" "letsencrypt_staging" {
  count = var.enable_cert_manager ? 1 : 0

  manifest = {
    apiVersion = "cert-manager.io/v1"
    kind       = "ClusterIssuer"
    metadata = {
      name = "letsencrypt-staging"
    }
    spec = {
      acme = {
        server = "https://acme-staging-v02.api.letsencrypt.org/directory"
        email  = "admin@${var.domain}"
        privateKeySecretRef = {
          name = "letsencrypt-staging"
        }
        solvers = [
          {
            http01 = {
              ingress = {
                class = "nginx"
              }
            }
          }
        ]
      }
    }
  }

  depends_on = [helm_release.cert_manager]
}

# Cloudflare API credentials secret for DNS-01 challenge
resource "kubernetes_secret" "cloudflare_api_credentials" {
  count = var.enable_dns01_challenge && var.cloudflare_api_token != "" ? 1 : 0

  metadata {
    name      = "cloudflare-api-token-secret"
    namespace = "cert-manager"
  }

  data = {
    "api-token" = var.cloudflare_api_token
  }

  type = "Opaque"
}

# DNS-01 ClusterIssuer for Let's Encrypt with Cloudflare
resource "kubernetes_manifest" "letsencrypt_prod_cloudflare" {
  count = var.enable_dns01_challenge && var.cloudflare_api_token != "" ? 1 : 0

  manifest = {
    apiVersion = "cert-manager.io/v1"
    kind       = "ClusterIssuer"
    metadata = {
      name = "letsencrypt-prod-cloudflare"
    }
    spec = {
      acme = {
        server = "https://acme-v02.api.letsencrypt.org/directory"
        email  = "admin@${var.domain}"
        privateKeySecretRef = {
          name = "letsencrypt-prod-cloudflare"
        }
        solvers = [
          {
            dns01 = {
              cloudflare = {
                apiTokenSecretRef = {
                  name = "cloudflare-api-token-secret"
                  key  = "api-token"
                }
              }
            }
          }
        ]
      }
    }
  }

  depends_on = [helm_release.cert_manager, kubernetes_secret.cloudflare_api_credentials]
}

# DNS-01 ClusterIssuer for Let's Encrypt staging with Cloudflare (for testing)
resource "kubernetes_manifest" "letsencrypt_staging_cloudflare" {
  count = var.enable_dns01_challenge && var.cloudflare_api_token != "" ? 1 : 0

  manifest = {
    apiVersion = "cert-manager.io/v1"
    kind       = "ClusterIssuer"
    metadata = {
      name = "letsencrypt-staging-cloudflare"
    }
    spec = {
      acme = {
        server = "https://acme-staging-v02.api.letsencrypt.org/directory"
        email  = "admin@${var.domain}"
        privateKeySecretRef = {
          name = "letsencrypt-staging-cloudflare"
        }
        solvers = [
          {
            dns01 = {
              cloudflare = {
                apiTokenSecretRef = {
                  name = "cloudflare-api-token-secret"
                  key  = "api-token"
                }
              }
            }
          }
        ]
      }
    }
  }

  depends_on = [helm_release.cert_manager, kubernetes_secret.cloudflare_api_credentials]
}

# Main application ingress
resource "kubernetes_ingress_v1" "econgraph" {
  metadata {
    name      = "econgraph-ingress"
    namespace = var.namespace
    annotations = {
      "kubernetes.io/ingress.class"                = "nginx"
      "nginx.ingress.kubernetes.io/rewrite-target" = "/"
      "nginx.ingress.kubernetes.io/ssl-redirect"   = "true"
      "nginx.ingress.kubernetes.io/force-ssl-redirect" = "true"

      # SSL configuration
      "cert-manager.io/cluster-issuer" = var.enable_cert_manager ? "letsencrypt-prod" : ""
      "nginx.ingress.kubernetes.io/ssl-protocols" = "TLSv1.2 TLSv1.3"

      # Security headers
      "nginx.ingress.kubernetes.io/configuration-snippet" = <<-EOT
        more_set_headers "X-Frame-Options: SAMEORIGIN";
        more_set_headers "X-Content-Type-Options: nosniff";
        more_set_headers "X-XSS-Protection: 1; mode=block";
        more_set_headers "Referrer-Policy: strict-origin-when-cross-origin";
        more_set_headers "Content-Security-Policy: default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self' wss:";
      EOT

      # Rate limiting
      "nginx.ingress.kubernetes.io/rate-limit" = "100"
      "nginx.ingress.kubernetes.io/rate-limit-window" = "1m"

      # Proxy configuration
      "nginx.ingress.kubernetes.io/proxy-body-size" = "50m"
      "nginx.ingress.kubernetes.io/proxy-read-timeout" = "300"
      "nginx.ingress.kubernetes.io/proxy-send-timeout" = "300"
    }
  }

  spec {
    # TLS configuration
    dynamic "tls" {
      for_each = var.enable_cert_manager ? [1] : []
      content {
        hosts       = [var.domain, "api.${var.domain}"]
        secret_name = "econgraph-tls"
      }
    }

    # Frontend rule
    rule {
      host = var.domain
      http {
        path {
          path      = "/"
          path_type = "Prefix"
          backend {
            service {
              name = "econgraph-frontend"
              port {
                number = 80
              }
            }
          }
        }
      }
    }

    # Backend API rule
    rule {
      host = "api.${var.domain}"
      http {
        path {
          path      = "/"
          path_type = "Prefix"
          backend {
            service {
              name = "econgraph-backend"
              port {
                number = 80
              }
            }
          }
        }
      }
    }
  }

  depends_on = [helm_release.nginx_ingress]
}

# Monitoring ingress (Grafana)
resource "kubernetes_ingress_v1" "monitoring" {
  metadata {
    name      = "monitoring-ingress"
    namespace = "${var.namespace}-monitoring"
    annotations = {
      "kubernetes.io/ingress.class"                = "nginx"
      "nginx.ingress.kubernetes.io/rewrite-target" = "/"
      "nginx.ingress.kubernetes.io/ssl-redirect"   = "true"
      "nginx.ingress.kubernetes.io/force-ssl-redirect" = "true"

      # SSL configuration
      "cert-manager.io/cluster-issuer" = var.enable_cert_manager ? "letsencrypt-prod" : ""

      # Authentication (basic auth for additional security)
      "nginx.ingress.kubernetes.io/auth-type" = "basic"
      "nginx.ingress.kubernetes.io/auth-secret" = "monitoring-auth"
      "nginx.ingress.kubernetes.io/auth-realm" = "EconGraph Monitoring"
    }
  }

  spec {
    # TLS configuration
    dynamic "tls" {
      for_each = var.enable_cert_manager ? [1] : []
      content {
        hosts       = ["grafana.${var.domain}"]
        secret_name = "monitoring-tls"
      }
    }

    # Grafana rule
    rule {
      host = "grafana.${var.domain}"
      http {
        path {
          path      = "/"
          path_type = "Prefix"
          backend {
            service {
              name = "prometheus-grafana"
              port {
                number = 80
              }
            }
          }
        }
      }
    }
  }

  depends_on = [helm_release.nginx_ingress]
}

# Basic auth secret for monitoring
resource "kubernetes_secret" "monitoring_auth" {
  metadata {
    name      = "monitoring-auth"
    namespace = "${var.namespace}-monitoring"
  }

  data = {
    # Username: admin, Password: admin123 (change in production!)
    auth = base64encode("admin:$2y$10$2b2cu8Fw7FoTN.oJCjRYEuVGmWBJPJgJoJGwFJJEWKCQKCvZKOqiC")
  }

  type = "Opaque"
}

# Network policy for ingress traffic
resource "kubernetes_network_policy" "ingress_policy" {
  metadata {
    name      = "ingress-network-policy"
    namespace = var.namespace
  }

  spec {
    pod_selector {
      match_labels = {
        app = "econgraph-frontend"
      }
    }

    policy_types = ["Ingress"]

    ingress {
      from {
        namespace_selector {
          match_labels = {
            name = "ingress-nginx"
          }
        }
      }
      ports {
        port     = "80"
        protocol = "TCP"
      }
    }
  }
}

# Outputs
output "ingress_ip" {
  description = "Ingress controller external IP"
  value       = helm_release.nginx_ingress.status[0].load_balancer[0].ingress[0].ip
}

output "application_urls" {
  description = "Application URLs"
  value = {
    frontend = var.enable_cert_manager ? "https://${var.domain}" : "http://${var.domain}"
    backend  = var.enable_cert_manager ? "https://api.${var.domain}" : "http://api.${var.domain}"
    grafana  = var.enable_cert_manager ? "https://grafana.${var.domain}" : "http://grafana.${var.domain}"
  }
}

output "cert_manager_enabled" {
  description = "Whether cert-manager is enabled"
  value       = var.enable_cert_manager
}
