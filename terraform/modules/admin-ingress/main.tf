# REQUIREMENT: Secure admin ingress with strict access controls
# PURPOSE: Provide controlled external access to admin interface with IP whitelisting
# This ensures administrative interface is only accessible from authorized networks

terraform {
  required_providers {
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "~> 2.23"
    }
  }
}

# Admin-specific ingress class (separate from public ingress)
resource "kubernetes_ingress_class_v1" "admin" {
  metadata {
    name = "nginx-admin"
    annotations = {
      "ingressclass.kubernetes.io/is-default-class" = "false"
      "description" = "Ingress class for administrative interfaces"
    }
  }

  spec {
    controller = "k8s.io/ingress-nginx-admin"
  }
}

# ConfigMap for admin NGINX configuration
resource "kubernetes_config_map" "admin_nginx_config" {
  metadata {
    name      = "admin-nginx-config"
    namespace = "ingress-nginx-admin"
  }

  data = {
    # Strict security configuration for admin access
    "nginx.conf" = <<-EOF
      # Admin-specific NGINX configuration with enhanced security

      # Rate limiting for admin access
      limit_req_zone $binary_remote_addr zone=admin_login:10m rate=5r/m;
      limit_req_zone $binary_remote_addr zone=admin_api:10m rate=30r/m;

      # Security headers
      add_header X-Frame-Options "DENY" always;
      add_header X-Content-Type-Options "nosniff" always;
      add_header X-XSS-Protection "1; mode=block" always;
      add_header Referrer-Policy "no-referrer" always;
      add_header Content-Security-Policy "default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self'; connect-src 'self'; frame-ancestors 'none';" always;
      add_header Strict-Transport-Security "max-age=31536000; includeSubDomains; preload" always;

      # Hide server information
      server_tokens off;

      # Admin access logging
      access_log /var/log/nginx/admin_access.log combined;
      error_log /var/log/nginx/admin_error.log warn;
    EOF

    # IP whitelist configuration
    "admin-whitelist.conf" = join("\n", [
      for ip in var.allowed_admin_ips : "allow ${ip};"
    ])
  }
}

# Secret for admin TLS certificate
resource "kubernetes_secret" "admin_tls" {
  metadata {
    name      = "admin-tls-secret"
    namespace = var.admin_namespace
  }

  data = {
    "tls.crt" = var.admin_tls_cert
    "tls.key" = var.admin_tls_key
  }

  type = "kubernetes.io/tls"
}

# Admin ingress with strict security
resource "kubernetes_ingress_v1" "admin" {
  metadata {
    name      = "admin-ingress"
    namespace = var.admin_namespace

    annotations = {
      "kubernetes.io/ingress.class"                    = "nginx-admin"
      "nginx.ingress.kubernetes.io/ssl-redirect"       = "true"
      "nginx.ingress.kubernetes.io/force-ssl-redirect" = "true"

      # IP whitelisting
      "nginx.ingress.kubernetes.io/whitelist-source-range" = join(",", var.allowed_admin_ips)

      # Rate limiting
      "nginx.ingress.kubernetes.io/limit-rps"        = "10"
      "nginx.ingress.kubernetes.io/limit-connections" = "5"

      # Security headers
      "nginx.ingress.kubernetes.io/configuration-snippet" = <<-EOF
        more_set_headers "X-Frame-Options: DENY";
        more_set_headers "X-Content-Type-Options: nosniff";
        more_set_headers "X-XSS-Protection: 1; mode=block";
        more_set_headers "Referrer-Policy: no-referrer";
        more_set_headers "Permissions-Policy: geolocation=(), microphone=(), camera=()";

        # Admin access logging with additional details
        access_log /var/log/nginx/admin_detailed.log '$remote_addr - $remote_user [$time_local] "$request" $status $body_bytes_sent "$http_referer" "$http_user_agent" "$http_x_forwarded_for" "$request_time" "$upstream_response_time"';
      EOF

      # Authentication (if using external auth)
      "nginx.ingress.kubernetes.io/auth-url"    = "http://auth-service.${var.admin_namespace}.svc.cluster.local/auth"
      "nginx.ingress.kubernetes.io/auth-signin" = "https://${var.admin_domain}/login"

      # Custom error pages
      "nginx.ingress.kubernetes.io/custom-http-errors" = "403,404,500,502,503"
      "nginx.ingress.kubernetes.io/default-backend"    = "admin-error-pages"

      # Additional security
      "nginx.ingress.kubernetes.io/server-snippet" = <<-EOF
        # Disable server signature
        server_tokens off;

        # Security timeouts
        client_body_timeout 10s;
        client_header_timeout 10s;

        # Limit request size
        client_max_body_size 1m;

        # Block common attack patterns
        location ~* \.(php|asp|aspx|jsp)$ {
          return 403;
        }

        location ~* /\. {
          return 403;
        }

        location ~* /(wp-|wordpress|admin|phpmyadmin) {
          return 403;
        }
      EOF
    }
  }

  spec {
    ingress_class_name = kubernetes_ingress_class_v1.admin.metadata[0].name

    tls {
      hosts       = [var.admin_domain]
      secret_name = kubernetes_secret.admin_tls.metadata[0].name
    }

    rule {
      host = var.admin_domain

      http {
        path {
          path      = "/"
          path_type = "Prefix"

          backend {
            service {
              name = var.admin_service_name
              port {
                number = var.admin_service_port
              }
            }
          }
        }

        # Health check endpoint (less restrictive)
        path {
          path      = "/health"
          path_type = "Exact"

          backend {
            service {
              name = var.admin_service_name
              port {
                number = var.admin_service_port
              }
            }
          }
        }
      }
    }
  }
}

# Network policy for admin ingress
resource "kubernetes_network_policy" "admin_ingress" {
  metadata {
    name      = "admin-ingress-policy"
    namespace = var.admin_namespace
  }

  spec {
    pod_selector {
      match_labels = {
        app = "admin-frontend"
      }
    }

    policy_types = ["Ingress"]

    ingress {
      from {
        # Only allow from ingress controller
        namespace_selector {
          match_labels = {
            name = "ingress-nginx-admin"
          }
        }
      }

      from {
        # Allow from monitoring namespace
        namespace_selector {
          match_labels = {
            name = "monitoring"
          }
        }
      }

      ports {
        protocol = "TCP"
        port     = var.admin_service_port
      }
    }
  }
}

# Service monitor for admin ingress (Prometheus)
resource "kubernetes_manifest" "admin_ingress_monitor" {
  count = var.enable_monitoring ? 1 : 0

  manifest = {
    apiVersion = "monitoring.coreos.com/v1"
    kind       = "ServiceMonitor"

    metadata = {
      name      = "admin-ingress-monitor"
      namespace = var.admin_namespace
      labels = {
        app = "admin-ingress"
        monitoring = "enabled"
      }
    }

    spec = {
      selector = {
        matchLabels = {
          app = "admin-frontend"
        }
      }

      endpoints = [
        {
          port     = "http"
          path     = "/metrics"
          interval = "30s"
        }
      ]
    }
  }
}

# Alert rules for admin access monitoring
resource "kubernetes_manifest" "admin_security_alerts" {
  count = var.enable_monitoring ? 1 : 0

  manifest = {
    apiVersion = "monitoring.coreos.com/v1"
    kind       = "PrometheusRule"

    metadata = {
      name      = "admin-security-alerts"
      namespace = var.admin_namespace
      labels = {
        app = "admin-security"
        monitoring = "enabled"
      }
    }

    spec = {
      groups = [
        {
          name = "admin-security"
          rules = [
            {
              alert = "AdminUnauthorizedAccess"
              expr  = "increase(nginx_ingress_controller_requests_total{service=\"admin-frontend\",status=~\"4..\"}[5m]) > 5"
              for   = "1m"
              labels = {
                severity = "critical"
                category = "security"
              }
              annotations = {
                summary     = "Multiple unauthorized access attempts to admin interface"
                description = "{{ $value }} unauthorized access attempts detected in the last 5 minutes"
              }
            },
            {
              alert = "AdminHighLatency"
              expr  = "histogram_quantile(0.95, sum(rate(nginx_ingress_controller_request_duration_seconds_bucket{service=\"admin-frontend\"}[5m])) by (le)) > 2"
              for   = "2m"
              labels = {
                severity = "warning"
                category = "performance"
              }
              annotations = {
                summary     = "Admin interface experiencing high latency"
                description = "95th percentile latency is {{ $value }}s"
              }
            },
            {
              alert = "AdminServiceDown"
              expr  = "up{job=\"admin-frontend\"} == 0"
              for   = "1m"
              labels = {
                severity = "critical"
                category = "availability"
              }
              annotations = {
                summary     = "Admin interface is down"
                description = "Admin frontend service is not responding"
              }
            }
          ]
        }
      ]
    }
  }
}

# Error pages service for custom error handling
resource "kubernetes_deployment" "admin_error_pages" {
  metadata {
    name      = "admin-error-pages"
    namespace = var.admin_namespace

    labels = {
      app = "admin-error-pages"
    }
  }

  spec {
    replicas = 1

    selector {
      match_labels = {
        app = "admin-error-pages"
      }
    }

    template {
      metadata {
        labels = {
          app = "admin-error-pages"
        }
      }

      spec {
        container {
          name  = "error-pages"
          image = "nginx:alpine"

          port {
            container_port = 80
          }

          volume_mount {
            name       = "error-pages"
            mount_path = "/usr/share/nginx/html"
          }
        }

        volume {
          name = "error-pages"
          config_map {
            name = kubernetes_config_map.admin_error_pages.metadata[0].name
          }
        }
      }
    }
  }
}

# Error pages content
resource "kubernetes_config_map" "admin_error_pages" {
  metadata {
    name      = "admin-error-pages"
    namespace = var.admin_namespace
  }

  data = {
    "403.html" = <<-EOF
      <!DOCTYPE html>
      <html>
      <head>
          <title>Access Denied</title>
          <style>
              body { font-family: Arial, sans-serif; text-align: center; margin-top: 50px; background: #f5f5f5; }
              .container { max-width: 600px; margin: 0 auto; padding: 20px; background: white; border-radius: 8px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }
              .error-code { font-size: 72px; color: #d32f2f; font-weight: bold; }
              .error-message { font-size: 24px; color: #333; margin: 20px 0; }
              .error-details { color: #666; margin: 20px 0; }
              .warning { background: #fff3cd; border: 1px solid #ffeaa7; padding: 15px; border-radius: 4px; margin: 20px 0; }
          </style>
      </head>
      <body>
          <div class="container">
              <div class="error-code">🚫 403</div>
              <div class="error-message">Access Denied</div>
              <div class="error-details">
                  You do not have permission to access this administrative interface.
              </div>
              <div class="warning">
                  <strong>Security Notice:</strong> This access attempt has been logged and reported to system administrators.
                  Unauthorized access attempts may result in legal action.
              </div>
          </div>
      </body>
      </html>
    EOF

    "404.html" = <<-EOF
      <!DOCTYPE html>
      <html>
      <head>
          <title>Page Not Found</title>
          <style>
              body { font-family: Arial, sans-serif; text-align: center; margin-top: 50px; background: #f5f5f5; }
              .container { max-width: 600px; margin: 0 auto; padding: 20px; background: white; border-radius: 8px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }
              .error-code { font-size: 72px; color: #ff9800; font-weight: bold; }
              .error-message { font-size: 24px; color: #333; margin: 20px 0; }
          </style>
      </head>
      <body>
          <div class="container">
              <div class="error-code">404</div>
              <div class="error-message">Administrative Resource Not Found</div>
              <p>The requested administrative resource could not be found.</p>
          </div>
      </body>
      </html>
    EOF

    "50x.html" = <<-EOF
      <!DOCTYPE html>
      <html>
      <head>
          <title>Service Unavailable</title>
          <style>
              body { font-family: Arial, sans-serif; text-align: center; margin-top: 50px; background: #f5f5f5; }
              .container { max-width: 600px; margin: 0 auto; padding: 20px; background: white; border-radius: 8px; box-shadow: 0 2px 10px rgba(0,0,0,0.1); }
              .error-code { font-size: 72px; color: #d32f2f; font-weight: bold; }
              .error-message { font-size: 24px; color: #333; margin: 20px 0; }
          </style>
      </head>
      <body>
          <div class="container">
              <div class="error-code">503</div>
              <div class="error-message">Administrative Interface Unavailable</div>
              <p>The administrative interface is temporarily unavailable. Please try again later.</p>
              <p>If this problem persists, please contact the system administrator.</p>
          </div>
      </body>
      </html>
    EOF
  }
}

# Service for error pages
resource "kubernetes_service" "admin_error_pages" {
  metadata {
    name      = "admin-error-pages"
    namespace = var.admin_namespace
  }

  spec {
    selector = {
      app = "admin-error-pages"
    }

    port {
      port        = 80
      target_port = 80
    }

    type = "ClusterIP"
  }
}
