# GraphQL Security Deployment Guide

## Overview

This guide provides comprehensive instructions for deploying the GraphQL security system in various environments, including development, staging, and production.

## Prerequisites

### System Requirements

- **Rust**: Version 1.70 or higher
- **PostgreSQL**: Version 13 or higher
- **Memory**: Minimum 512MB RAM
- **CPU**: Minimum 1 core
- **Storage**: Minimum 1GB available space

### Dependencies

- `async-graphql` 7.0
- `warp` web framework
- `diesel` ORM
- `tokio` async runtime
- `serde` serialization
- `chrono` date/time handling
- `uuid` unique identifiers
- `regex` pattern matching

## Environment Setup

### Development Environment

#### 1. Clone Repository

```bash
git clone https://github.com/your-org/econ-graph4.git
cd econ-graph4
```

#### 2. Install Dependencies

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Install PostgreSQL
# macOS
brew install postgresql
brew services start postgresql

# Ubuntu/Debian
sudo apt-get update
sudo apt-get install postgresql postgresql-contrib

# CentOS/RHEL
sudo yum install postgresql-server postgresql-contrib
sudo postgresql-setup initdb
sudo systemctl start postgresql
```

#### 3. Database Setup

```bash
# Create database
createdb econ_graph_dev

# Run migrations
cd backend
diesel migration run
```

#### 4. Environment Variables

Create `.env` file:

```bash
# Database
DATABASE_URL=postgresql://username:password@localhost/econ_graph_dev

# Security Configuration
SECURITY_MAX_COMPLEXITY=2000
SECURITY_MAX_DEPTH=15
SECURITY_MAX_QUERY_SIZE=20000
SECURITY_QUERY_TIMEOUT=60
SECURITY_PROTECT_INTROSPECTION=false

# Rate Limiting
RATE_LIMIT_REQUESTS_PER_MINUTE=120
RATE_LIMIT_REQUESTS_PER_HOUR=2000
RATE_LIMIT_REQUESTS_PER_DAY=20000
RATE_LIMIT_ENABLED=false

# Input Validation
INPUT_VALIDATION_MAX_STRING_LENGTH=2000
INPUT_VALIDATION_ENABLE_HTML_SANITIZATION=true
INPUT_VALIDATION_ENABLE_SQL_INJECTION_PREVENTION=true
INPUT_VALIDATION_ENABLE_XSS_PREVENTION=true

# Logging
RUST_LOG=debug
LOG_LEVEL=debug
```

#### 5. Build and Run

```bash
# Build the project
cargo build

# Run in development mode
cargo run --bin econ-graph-graphql
```

### Staging Environment

#### 1. Server Setup

```bash
# Create staging user
sudo useradd -m -s /bin/bash econ-graph-staging
sudo usermod -aG sudo econ-graph-staging

# Switch to staging user
sudo su - econ-graph-staging
```

#### 2. Application Directory

```bash
# Create application directory
mkdir -p /home/econ-graph-staging/app
cd /home/econ-graph-staging/app

# Clone repository
git clone https://github.com/your-org/econ-graph4.git .
git checkout staging
```

#### 3. Database Setup

```bash
# Create staging database
sudo -u postgres createdb econ_graph_staging

# Set up database user
sudo -u postgres psql -c "CREATE USER econ_graph_staging WITH PASSWORD 'secure_password';"
sudo -u postgres psql -c "GRANT ALL PRIVILEGES ON DATABASE econ_graph_staging TO econ_graph_staging;"
```

#### 4. Environment Configuration

Create `/home/econ-graph-staging/app/.env`:

```bash
# Database
DATABASE_URL=postgresql://econ_graph_staging:secure_password@localhost/econ_graph_staging

# Security Configuration
SECURITY_MAX_COMPLEXITY=1000
SECURITY_MAX_DEPTH=10
SECURITY_MAX_QUERY_SIZE=10000
SECURITY_QUERY_TIMEOUT=30
SECURITY_PROTECT_INTROSPECTION=true

# Rate Limiting
RATE_LIMIT_REQUESTS_PER_MINUTE=60
RATE_LIMIT_REQUESTS_PER_HOUR=1000
RATE_LIMIT_REQUESTS_PER_DAY=10000
RATE_LIMIT_ENABLED=true

# Input Validation
INPUT_VALIDATION_MAX_STRING_LENGTH=1000
INPUT_VALIDATION_ENABLE_HTML_SANITIZATION=true
INPUT_VALIDATION_ENABLE_SQL_INJECTION_PREVENTION=true
INPUT_VALIDATION_ENABLE_XSS_PREVENTION=true

# Logging
RUST_LOG=info
LOG_LEVEL=info
```

#### 5. Build and Deploy

```bash
# Build release version
cargo build --release

# Run migrations
diesel migration run

# Start application
./target/release/econ-graph-graphql
```

### Production Environment

#### 1. Server Setup

```bash
# Create production user
sudo useradd -m -s /bin/bash econ-graph-prod
sudo usermod -aG sudo econ-graph-prod

# Switch to production user
sudo su - econ-graph-prod
```

#### 2. Application Directory

```bash
# Create application directory
mkdir -p /home/econ-graph-prod/app
cd /home/econ-graph-prod/app

# Clone repository
git clone https://github.com/your-org/econ-graph4.git .
git checkout main
```

#### 3. Database Setup

```bash
# Create production database
sudo -u postgres createdb econ_graph_prod

# Set up database user
sudo -u postgres psql -c "CREATE USER econ_graph_prod WITH PASSWORD 'very_secure_password';"
sudo -u postgres psql -c "GRANT ALL PRIVILEGES ON DATABASE econ_graph_prod TO econ_graph_prod;"
```

#### 4. Environment Configuration

Create `/home/econ-graph-prod/app/.env`:

```bash
# Database
DATABASE_URL=postgresql://econ_graph_prod:very_secure_password@localhost/econ_graph_prod

# Security Configuration
SECURITY_MAX_COMPLEXITY=500
SECURITY_MAX_DEPTH=8
SECURITY_MAX_QUERY_SIZE=5000
SECURITY_QUERY_TIMEOUT=15
SECURITY_PROTECT_INTROSPECTION=true

# Rate Limiting
RATE_LIMIT_REQUESTS_PER_MINUTE=30
RATE_LIMIT_REQUESTS_PER_HOUR=500
RATE_LIMIT_REQUESTS_PER_DAY=5000
RATE_LIMIT_ENABLED=true

# Input Validation
INPUT_VALIDATION_MAX_STRING_LENGTH=500
INPUT_VALIDATION_ENABLE_HTML_SANITIZATION=true
INPUT_VALIDATION_ENABLE_SQL_INJECTION_PREVENTION=true
INPUT_VALIDATION_ENABLE_XSS_PREVENTION=true

# Logging
RUST_LOG=warn
LOG_LEVEL=warn
```

#### 5. Build and Deploy

```bash
# Build release version
cargo build --release

# Run migrations
diesel migration run

# Start application
./target/release/econ-graph-graphql
```

## Docker Deployment

### Dockerfile

Create `Dockerfile`:

```dockerfile
# Build stage
FROM rust:1.70 as builder

WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

# Build dependencies
RUN cargo build --release

# Copy source code
COPY . .

# Build application
RUN cargo build --release --bin econ-graph-graphql

# Runtime stage
FROM debian:bullseye-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl1.1 \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -r -s /bin/false econ-graph

# Copy binary
COPY --from=builder /app/target/release/econ-graph-graphql /usr/local/bin/

# Set ownership
RUN chown econ-graph:econ-graph /usr/local/bin/econ-graph-graphql

# Switch to app user
USER econ-graph

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/health || exit 1

# Start application
CMD ["econ-graph-graphql"]
```

### Docker Compose

Create `docker-compose.yml`:

```yaml
version: '3.8'

services:
  postgres:
    image: postgres:13
    environment:
      POSTGRES_DB: econ_graph
      POSTGRES_USER: econ_graph
      POSTGRES_PASSWORD: secure_password
    volumes:
      - postgres_data:/var/lib/postgresql/data
    ports:
      - "5432:5432"
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U econ_graph"]
      interval: 10s
      timeout: 5s
      retries: 5

  econ-graph-graphql:
    build: .
    environment:
      DATABASE_URL: postgresql://econ_graph:secure_password@postgres:5432/econ_graph
      SECURITY_MAX_COMPLEXITY: 1000
      SECURITY_MAX_DEPTH: 10
      SECURITY_MAX_QUERY_SIZE: 10000
      SECURITY_QUERY_TIMEOUT: 30
      SECURITY_PROTECT_INTROSPECTION: true
      RATE_LIMIT_REQUESTS_PER_MINUTE: 60
      RATE_LIMIT_REQUESTS_PER_HOUR: 1000
      RATE_LIMIT_REQUESTS_PER_DAY: 10000
      RATE_LIMIT_ENABLED: true
      RUST_LOG: info
    ports:
      - "8080:8080"
    depends_on:
      postgres:
        condition: service_healthy
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 10s
      retries: 3
    restart: unless-stopped

volumes:
  postgres_data:
```

### Build and Run

```bash
# Build and start services
docker-compose up -d

# View logs
docker-compose logs -f econ-graph-graphql

# Stop services
docker-compose down
```

## Kubernetes Deployment

### Namespace

Create `k8s/namespace.yaml`:

```yaml
apiVersion: v1
kind: Namespace
metadata:
  name: econ-graph
  labels:
    name: econ-graph
```

### ConfigMap

Create `k8s/configmap.yaml`:

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: econ-graph-config
  namespace: econ-graph
data:
  SECURITY_MAX_COMPLEXITY: "1000"
  SECURITY_MAX_DEPTH: "10"
  SECURITY_MAX_QUERY_SIZE: "10000"
  SECURITY_QUERY_TIMEOUT: "30"
  SECURITY_PROTECT_INTROSPECTION: "true"
  RATE_LIMIT_REQUESTS_PER_MINUTE: "60"
  RATE_LIMIT_REQUESTS_PER_HOUR: "1000"
  RATE_LIMIT_REQUESTS_PER_DAY: "10000"
  RATE_LIMIT_ENABLED: "true"
  RUST_LOG: "info"
```

### Secret

Create `k8s/secret.yaml`:

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: econ-graph-secret
  namespace: econ-graph
type: Opaque
data:
  DATABASE_URL: cG9zdGdyZXNxbDovL2Vjb25fZ3JhcGg6c2VjdXJlX3Bhc3N3b3JkQGxvY2FsaG9zdC9lY29uX2dyYXBo
```

### Deployment

Create `k8s/deployment.yaml`:

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: econ-graph-graphql
  namespace: econ-graph
  labels:
    app: econ-graph-graphql
spec:
  replicas: 3
  selector:
    matchLabels:
      app: econ-graph-graphql
  template:
    metadata:
      labels:
        app: econ-graph-graphql
    spec:
      containers:
      - name: econ-graph-graphql
        image: econ-graph-graphql:latest
        ports:
        - containerPort: 8080
        env:
        - name: DATABASE_URL
          valueFrom:
            secretKeyRef:
              name: econ-graph-secret
              key: DATABASE_URL
        envFrom:
        - configMapRef:
            name: econ-graph-config
        resources:
          requests:
            memory: "512Mi"
            cpu: "250m"
          limits:
            memory: "1Gi"
            cpu: "500m"
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 10
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 5
          periodSeconds: 5
        securityContext:
          runAsNonRoot: true
          runAsUser: 1000
          allowPrivilegeEscalation: false
          readOnlyRootFilesystem: true
          capabilities:
            drop:
            - ALL
```

### Service

Create `k8s/service.yaml`:

```yaml
apiVersion: v1
kind: Service
metadata:
  name: econ-graph-graphql-service
  namespace: econ-graph
spec:
  selector:
    app: econ-graph-graphql
  ports:
  - protocol: TCP
    port: 80
    targetPort: 8080
  type: ClusterIP
```

### Ingress

Create `k8s/ingress.yaml`:

```yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: econ-graph-graphql-ingress
  namespace: econ-graph
  annotations:
    nginx.ingress.kubernetes.io/rewrite-target: /
    nginx.ingress.kubernetes.io/ssl-redirect: "true"
    nginx.ingress.kubernetes.io/rate-limit: "100"
    nginx.ingress.kubernetes.io/rate-limit-window: "1m"
spec:
  tls:
  - hosts:
    - api.econ-graph.com
    secretName: econ-graph-tls
  rules:
  - host: api.econ-graph.com
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: econ-graph-graphql-service
            port:
              number: 80
```

### Deploy to Kubernetes

```bash
# Apply all configurations
kubectl apply -f k8s/

# Check deployment status
kubectl get pods -n econ-graph

# View logs
kubectl logs -f deployment/econ-graph-graphql -n econ-graph

# Scale deployment
kubectl scale deployment econ-graph-graphql --replicas=5 -n econ-graph
```

## Load Balancer Configuration

### Nginx Configuration

Create `nginx.conf`:

```nginx
upstream econ_graph_backend {
    server 127.0.0.1:8080;
    server 127.0.0.1:8081;
    server 127.0.0.1:8082;
}

server {
    listen 80;
    server_name api.econ-graph.com;

    # Rate limiting
    limit_req_zone $binary_remote_addr zone=api:10m rate=10r/s;
    limit_req zone=api burst=20 nodelay;

    # Security headers
    add_header X-Frame-Options DENY;
    add_header X-Content-Type-Options nosniff;
    add_header X-XSS-Protection "1; mode=block";
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains";

    # GraphQL endpoint
    location /graphql {
        proxy_pass http://econ_graph_backend;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # Timeout settings
        proxy_connect_timeout 30s;
        proxy_send_timeout 30s;
        proxy_read_timeout 30s;
        
        # Buffer settings
        proxy_buffering on;
        proxy_buffer_size 4k;
        proxy_buffers 8 4k;
    }

    # Health check endpoint
    location /health {
        proxy_pass http://econ_graph_backend;
        access_log off;
    }

    # Metrics endpoint
    location /metrics {
        proxy_pass http://econ_graph_backend;
        allow 127.0.0.1;
        deny all;
    }
}
```

### HAProxy Configuration

Create `haproxy.cfg`:

```haproxy
global
    daemon
    user haproxy
    group haproxy
    log 127.0.0.1:514 local0
    chroot /var/lib/haproxy
    stats socket /run/haproxy/admin.sock mode 660 level admin
    stats timeout 30s

defaults
    mode http
    log global
    option httplog
    option dontlognull
    option log-health-checks
    option forwardfor
    option httpchk GET /health
    timeout connect 5000
    timeout client 50000
    timeout server 50000
    errorfile 400 /etc/haproxy/errors/400.http
    errorfile 403 /etc/haproxy/errors/403.http
    errorfile 408 /etc/haproxy/errors/408.http
    errorfile 500 /etc/haproxy/errors/500.http
    errorfile 502 /etc/haproxy/errors/502.http
    errorfile 503 /etc/haproxy/errors/503.http
    errorfile 504 /etc/haproxy/errors/504.http

frontend econ_graph_frontend
    bind *:80
    bind *:443 ssl crt /etc/ssl/certs/econ-graph.pem
    
    # Rate limiting
    stick-table type ip size 100k expire 30s store http_req_rate(10s)
    http-request track-sc0 src
    http-request deny if { sc_http_req_rate(0) gt 10 }
    
    # Security headers
    http-response set-header X-Frame-Options DENY
    http-response set-header X-Content-Type-Options nosniff
    http-response set-header X-XSS-Protection "1; mode=block"
    http-response set-header Strict-Transport-Security "max-age=31536000; includeSubDomains"
    
    # Routing
    default_backend econ_graph_backend

backend econ_graph_backend
    balance roundrobin
    option httpchk GET /health
    
    server econ_graph_1 127.0.0.1:8080 check
    server econ_graph_2 127.0.0.1:8081 check
    server econ_graph_3 127.0.0.1:8082 check

listen stats
    bind *:8404
    stats enable
    stats uri /stats
    stats refresh 30s
    stats admin if TRUE
```

## Monitoring and Observability

### Prometheus Configuration

Create `prometheus.yml`:

```yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

rule_files:
  - "econ_graph_rules.yml"

scrape_configs:
  - job_name: 'econ-graph-graphql'
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: '/metrics'
    scrape_interval: 5s
    scrape_timeout: 5s

  - job_name: 'postgres'
    static_configs:
      - targets: ['localhost:9187']
    scrape_interval: 15s

  - job_name: 'nginx'
    static_configs:
      - targets: ['localhost:9113']
    scrape_interval: 15s

alerting:
  alertmanagers:
    - static_configs:
        - targets:
          - alertmanager:9093
```

### Grafana Dashboard

Create `grafana-dashboard.json`:

```json
{
  "dashboard": {
    "id": null,
    "title": "Econ Graph GraphQL Security",
    "tags": ["econ-graph", "graphql", "security"],
    "timezone": "browser",
    "panels": [
      {
        "id": 1,
        "title": "Request Rate",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(econ_graph_requests_total[5m])",
            "legendFormat": "Requests/sec"
          }
        ]
      },
      {
        "id": 2,
        "title": "Security Events",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(econ_graph_security_events_total[5m])",
            "legendFormat": "Security Events/sec"
          }
        ]
      },
      {
        "id": 3,
        "title": "Query Complexity",
        "type": "graph",
        "targets": [
          {
            "expr": "histogram_quantile(0.95, rate(econ_graph_query_complexity_bucket[5m]))",
            "legendFormat": "95th percentile"
          }
        ]
      }
    ]
  }
}
```

### Alert Rules

Create `econ_graph_rules.yml`:

```yaml
groups:
- name: econ_graph_security
  rules:
  - alert: HighSecurityEventRate
    expr: rate(econ_graph_security_events_total[5m]) > 10
    for: 2m
    labels:
      severity: warning
    annotations:
      summary: "High security event rate detected"
      description: "Security events are occurring at {{ $value }} events/sec"

  - alert: QueryComplexityExceeded
    expr: rate(econ_graph_complexity_exceeded_total[5m]) > 5
    for: 1m
    labels:
      severity: critical
    annotations:
      summary: "Query complexity limit exceeded"
      description: "{{ $value }} queries exceeded complexity limit in the last 5 minutes"

  - alert: RateLimitExceeded
    expr: rate(econ_graph_rate_limit_exceeded_total[5m]) > 20
    for: 2m
    labels:
      severity: warning
    annotations:
      summary: "Rate limit exceeded frequently"
      description: "{{ $value }} requests exceeded rate limit in the last 5 minutes"

  - alert: ServiceDown
    expr: up{job="econ-graph-graphql"} == 0
    for: 1m
    labels:
      severity: critical
    annotations:
      summary: "Econ Graph GraphQL service is down"
      description: "The GraphQL service has been down for more than 1 minute"
```

## Security Hardening

### System Security

#### 1. Firewall Configuration

```bash
# UFW (Ubuntu)
sudo ufw enable
sudo ufw allow 22/tcp
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
sudo ufw deny 5432/tcp

# iptables (CentOS/RHEL)
sudo iptables -A INPUT -p tcp --dport 22 -j ACCEPT
sudo iptables -A INPUT -p tcp --dport 80 -j ACCEPT
sudo iptables -A INPUT -p tcp --dport 443 -j ACCEPT
sudo iptables -A INPUT -p tcp --dport 5432 -j DROP
sudo iptables -A INPUT -j DROP
```

#### 2. SSL/TLS Configuration

```bash
# Generate SSL certificate
openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 -nodes

# Configure SSL in application
SSL_CERT_PATH=/path/to/cert.pem
SSL_KEY_PATH=/path/to/key.pem
```

#### 3. Database Security

```sql
-- Create restricted database user
CREATE USER econ_graph_app WITH PASSWORD 'secure_password';
GRANT CONNECT ON DATABASE econ_graph TO econ_graph_app;
GRANT USAGE ON SCHEMA public TO econ_graph_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO econ_graph_app;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO econ_graph_app;

-- Revoke unnecessary privileges
REVOKE CREATE ON SCHEMA public FROM econ_graph_app;
REVOKE DROP ON SCHEMA public FROM econ_graph_app;
```

### Application Security

#### 1. Environment Variables

```bash
# Use strong passwords
DATABASE_PASSWORD=$(openssl rand -base64 32)

# Enable security features
SECURITY_PROTECT_INTROSPECTION=true
SECURITY_MAX_COMPLEXITY=500
SECURITY_MAX_DEPTH=8
RATE_LIMIT_ENABLED=true
```

#### 2. Input Validation

```rust
// Enable all input validation features
INPUT_VALIDATION_ENABLE_HTML_SANITIZATION=true
INPUT_VALIDATION_ENABLE_SQL_INJECTION_PREVENTION=true
INPUT_VALIDATION_ENABLE_XSS_PREVENTION=true
INPUT_VALIDATION_MAX_STRING_LENGTH=500
```

#### 3. Logging and Monitoring

```bash
# Enable security logging
RUST_LOG=warn
SECURITY_LOG_LEVEL=info
SECURITY_LOG_EVENTS=true

# Configure log rotation
logrotate /etc/logrotate.d/econ-graph
```

## Backup and Recovery

### Database Backup

```bash
# Create backup script
#!/bin/bash
BACKUP_DIR="/backups/econ-graph"
DATE=$(date +%Y%m%d_%H%M%S)
BACKUP_FILE="$BACKUP_DIR/econ_graph_$DATE.sql"

# Create backup
pg_dump -h localhost -U econ_graph_prod econ_graph_prod > $BACKUP_FILE

# Compress backup
gzip $BACKUP_FILE

# Remove old backups (keep 30 days)
find $BACKUP_DIR -name "*.sql.gz" -mtime +30 -delete

# Schedule in crontab
# 0 2 * * * /path/to/backup_script.sh
```

### Application Backup

```bash
# Backup application files
tar -czf /backups/econ-graph/app_$(date +%Y%m%d_%H%M%S).tar.gz /home/econ-graph-prod/app

# Backup configuration
cp /home/econ-graph-prod/app/.env /backups/econ-graph/config_$(date +%Y%m%d_%H%M%S).env
```

### Recovery Procedures

```bash
# Database recovery
gunzip -c /backups/econ-graph/econ_graph_20231201_020000.sql.gz | psql -h localhost -U econ_graph_prod econ_graph_prod

# Application recovery
tar -xzf /backups/econ-graph/app_20231201_020000.tar.gz -C /
cp /backups/econ-graph/config_20231201_020000.env /home/econ-graph-prod/app/.env
```

## Troubleshooting

### Common Issues

#### 1. High Memory Usage

```bash
# Check memory usage
free -h
ps aux --sort=-%mem | head

# Check for memory leaks
valgrind --tool=memcheck ./target/release/econ-graph-graphql

# Monitor memory over time
watch -n 1 'free -h'
```

#### 2. Performance Issues

```bash
# Check CPU usage
top -p $(pgrep econ-graph-graphql)

# Profile application
perf record -p $(pgrep econ-graph-graphql)
perf report

# Check database performance
pg_stat_statements
```

#### 3. Security Issues

```bash
# Check security logs
tail -f /var/log/econ-graph/security.log

# Monitor failed authentication attempts
grep "authentication failed" /var/log/postgresql/postgresql.log

# Check for suspicious queries
grep "suspicious" /var/log/econ-graph/security.log
```

### Debug Mode

```bash
# Enable debug logging
RUST_LOG=debug cargo run

# Enable trace logging
RUST_LOG=trace cargo run

# Enable specific module logging
RUST_LOG=econ_graph_graphql::security=debug cargo run
```

### Health Checks

```bash
# Application health
curl -f http://localhost:8080/health

# Database health
pg_isready -h localhost -p 5432

# Security metrics
curl http://localhost:8080/metrics | grep security
```

## Maintenance

### Regular Tasks

#### 1. Security Updates

```bash
# Update system packages
sudo apt update && sudo apt upgrade

# Update Rust
rustup update

# Update dependencies
cargo update
```

#### 2. Log Rotation

```bash
# Configure logrotate
sudo vim /etc/logrotate.d/econ-graph

# Manual log rotation
sudo logrotate -f /etc/logrotate.d/econ-graph
```

#### 3. Performance Monitoring

```bash
# Check system resources
htop
iotop
nethogs

# Check application metrics
curl http://localhost:8080/metrics
```

### Scaling

#### 1. Horizontal Scaling

```bash
# Add more instances
kubectl scale deployment econ-graph-graphql --replicas=5 -n econ-graph

# Update load balancer
kubectl apply -f k8s/ingress.yaml
```

#### 2. Vertical Scaling

```bash
# Increase resource limits
kubectl patch deployment econ-graph-graphql -n econ-graph -p '{"spec":{"template":{"spec":{"containers":[{"name":"econ-graph-graphql","resources":{"limits":{"memory":"2Gi","cpu":"1000m"}}}]}}}}'
```

## Conclusion

This deployment guide provides comprehensive instructions for deploying the GraphQL security system in various environments. The system is designed to be secure, scalable, and maintainable while providing robust security features and monitoring capabilities.

Key deployment considerations:

1. **Security First**: Always enable security features in production
2. **Monitoring**: Implement comprehensive monitoring and alerting
3. **Backup**: Regular backups of database and configuration
4. **Scaling**: Plan for horizontal and vertical scaling
5. **Maintenance**: Regular security updates and performance monitoring

Follow these guidelines to ensure a successful deployment and operation of the GraphQL security system.
