# MinIO Storage Tiering Design

## Overview

This document outlines the design for implementing MinIO-based storage tiering in the financial data service. The system provides automatic and manual tiering across multiple storage types (NVMe, SSD, HDD, S3) with configurable policies and cost optimization.

## Architecture

### **Storage Tier Hierarchy**

```
┌────────────────────────────────────────┐
│           Financial Data Service       │
├────────────────────────────────────────┤
│  GraphQL API (Apollo Server)          │
│  Storage Tiering Manager              │
│  MinIO Client (Multi-tier)            │
└────────────────────────────────────────┘
                    │
        ┌───────────┼───────────┐
        │           │           │
┌───────▼───┐ ┌─────▼─────┐ ┌───▼────┐
│   Hot     │ │   Warm    │ │  Cold  │
│  (NVMe)   │ │   (SSD)   │ │ (HDD)  │
│ < 1ms     │ │ < 10ms    │ │ < 100ms│
└───────────┘ └───────────┘ └────────┘
        │           │           │
        └───────────┼───────────┘
                   │
            ┌──────▼──────┐
            │   Archive   │
            │    (S3)     │
            │   < 1s     │
            └─────────────┘
```

### **MinIO Configuration**

#### **Multi-Tier MinIO Setup**
```yaml
# docker-compose.yml
version: '3.8'
services:
  minio-hot:
    image: minio/minio:latest
    command: server /data --console-address ":9001"
    environment:
      MINIO_ROOT_USER: admin
      MINIO_ROOT_PASSWORD: password123
    volumes:
      - /mnt/nvme/data:/data
    ports:
      - "9000:9000"
      - "9001:9001"
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:9000/minio/health/live"]
      interval: 30s
      timeout: 20s
      retries: 3

  minio-warm:
    image: minio/minio:latest
    command: server /data --console-address ":9003"
    environment:
      MINIO_ROOT_USER: admin
      MINIO_ROOT_PASSWORD: password123
    volumes:
      - /mnt/ssd/data:/data
    ports:
      - "9002:9000"
      - "9003:9001"

  minio-cold:
    image: minio/minio:latest
    command: server /data --console-address ":9005"
    environment:
      MINIO_ROOT_USER: admin
      MINIO_ROOT_PASSWORD: password123
    volumes:
      - /mnt/hdd/data:/data
    ports:
      - "9004:9000"
      - "9005:9001"

  minio-archive:
    image: minio/minio:latest
    command: server /data --console-address ":9007"
    environment:
      MINIO_ROOT_USER: admin
      MINIO_ROOT_PASSWORD: password123
    volumes:
      - /mnt/archive/data:/data
    ports:
      - "9006:9000"
      - "9007:9001"
```

## Storage Tiering Implementation

### **Tiering Manager**

```rust
use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageTier {
    pub name: String,
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket: String,
    pub max_size: u64,
    pub latency_threshold: u64, // milliseconds
    pub cost_per_gb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessPattern {
    pub file_path: String,
    pub access_count: u32,
    pub last_access: DateTime<Utc>,
    pub access_frequency: f64, // accesses per day
    pub access_trend: AccessTrend,
    pub file_size: u64,
    pub current_tier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccessTrend {
    Increasing,
    Decreasing,
    Stable,
    Sporadic,
}

pub struct TieringManager {
    tiers: HashMap<String, StorageTier>,
    access_patterns: HashMap<String, AccessPattern>,
    tiering_policies: TieringPolicies,
}

impl TieringManager {
    pub fn new() -> Self {
        Self {
            tiers: HashMap::new(),
            access_patterns: HashMap::new(),
            tiering_policies: TieringPolicies::default(),
        }
    }

    pub fn add_tier(&mut self, tier: StorageTier) {
        self.tiers.insert(tier.name.clone(), tier);
    }

    pub async fn analyze_access_patterns(&mut self) -> Result<()> {
        // Analyze access patterns and determine optimal tiering
        for (file_path, pattern) in &self.access_patterns {
            let optimal_tier = self.determine_optimal_tier(pattern);
            if optimal_tier != pattern.current_tier {
                self.schedule_tier_migration(file_path, &optimal_tier).await?;
            }
        }
        Ok(())
    }

    fn determine_optimal_tier(&self, pattern: &AccessPattern) -> String {
        match pattern.access_frequency {
            f if f > 10.0 => "hot".to_string(),      // Daily access
            f if f > 1.0 => "warm".to_string(),     // Weekly access
            f if f > 0.1 => "cold".to_string(),     // Monthly access
            _ => "archive".to_string(),              // Rarely accessed
        }
    }

    pub async fn schedule_tier_migration(&self, file_path: &str, target_tier: &str) -> Result<()> {
        // Schedule file migration to target tier
        // This would integrate with MinIO client for actual file movement
        println!("Migrating {} to {} tier", file_path, target_tier);
        Ok(())
    }
}
```

### **Tiering Policies**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TieringPolicies {
    pub automatic_tiering: bool,
    pub frequency_thresholds: FrequencyThresholds,
    pub time_based_tiering: TimeBasedTiering,
    pub cost_optimization: CostOptimization,
    pub manual_overrides: Vec<ManualOverride>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrequencyThresholds {
    pub hot_to_warm: f64,      // accesses per day
    pub warm_to_cold: f64,
    pub cold_to_archive: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeBasedTiering {
    pub hot_retention_days: u32,
    pub warm_retention_days: u32,
    pub cold_retention_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostOptimization {
    pub max_hot_storage_gb: u64,
    pub max_warm_storage_gb: u64,
    pub archive_after_days: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualOverride {
    pub file_pattern: String,
    pub target_tier: String,
    pub priority: u32,
}

impl Default for TieringPolicies {
    fn default() -> Self {
        Self {
            automatic_tiering: true,
            frequency_thresholds: FrequencyThresholds {
                hot_to_warm: 5.0,
                warm_to_cold: 1.0,
                cold_to_archive: 0.1,
            },
            time_based_tiering: TimeBasedTiering {
                hot_retention_days: 7,
                warm_retention_days: 30,
                cold_retention_days: 365,
            },
            cost_optimization: CostOptimization {
                max_hot_storage_gb: 1000,
                max_warm_storage_gb: 10000,
                archive_after_days: 1095, // 3 years
            },
            manual_overrides: Vec::new(),
        }
    }
}
```

## Configuration

### **Storage Tier Configuration**

```yaml
# config/storage-tiers.yml
storage:
  tiers:
    hot:
      name: "hot"
      endpoint: "http://localhost:9000"
      access_key: "admin"
      secret_key: "password123"
      bucket: "financial-data-hot"
      max_size_gb: 1000
      latency_threshold_ms: 1
      cost_per_gb: 0.10
      
    warm:
      name: "warm"
      endpoint: "http://localhost:9002"
      access_key: "admin"
      secret_key: "password123"
      bucket: "financial-data-warm"
      max_size_gb: 10000
      latency_threshold_ms: 10
      cost_per_gb: 0.05
      
    cold:
      name: "cold"
      endpoint: "http://localhost:9004"
      access_key: "admin"
      secret_key: "password123"
      bucket: "financial-data-cold"
      max_size_gb: 100000
      latency_threshold_ms: 100
      cost_per_gb: 0.02
      
    archive:
      name: "archive"
      endpoint: "http://localhost:9006"
      access_key: "admin"
      secret_key: "password123"
      bucket: "financial-data-archive"
      max_size_gb: 1000000
      latency_threshold_ms: 1000
      cost_per_gb: 0.01

  policies:
    automatic_tiering: true
    frequency_thresholds:
      hot_to_warm: 5.0      # accesses per day
      warm_to_cold: 1.0
      cold_to_archive: 0.1
    time_based_tiering:
      hot_retention_days: 7
      warm_retention_days: 30
      cold_retention_days: 365
    cost_optimization:
      max_hot_storage_gb: 1000
      max_warm_storage_gb: 10000
      archive_after_days: 1095
```

### **Docker Compose with Storage Volumes**

```yaml
# docker-compose.storage.yml
version: '3.8'
services:
  financial-data-service:
    build: .
    environment:
      - STORAGE_CONFIG_PATH=/app/config/storage-tiers.yml
    volumes:
      - ./config:/app/config
      - /mnt/nvme:/mnt/nvme
      - /mnt/ssd:/mnt/ssd
      - /mnt/hdd:/mnt/hdd
      - /mnt/archive:/mnt/archive
    depends_on:
      - minio-hot
      - minio-warm
      - minio-cold
      - minio-archive

  minio-hot:
    image: minio/minio:latest
    command: server /data --console-address ":9001"
    environment:
      MINIO_ROOT_USER: admin
      MINIO_ROOT_PASSWORD: password123
    volumes:
      - /mnt/nvme/data:/data
    ports:
      - "9000:9000"
      - "9001:9001"

  minio-warm:
    image: minio/minio:latest
    command: server /data --console-address ":9003"
    environment:
      MINIO_ROOT_USER: admin
      MINIO_ROOT_PASSWORD: password123
    volumes:
      - /mnt/ssd/data:/data
    ports:
      - "9002:9000"
      - "9003:9001"

  minio-cold:
    image: minio/minio:latest
    command: server /data --console-address ":9005"
    environment:
      MINIO_ROOT_USER: admin
      MINIO_ROOT_PASSWORD: password123
    volumes:
      - /mnt/hdd/data:/data
    ports:
      - "9004:9000"
      - "9005:9001"

  minio-archive:
    image: minio/minio:latest
    command: server /data --console-address ":9007"
    environment:
      MINIO_ROOT_USER: admin
      MINIO_ROOT_PASSWORD: password123
    volumes:
      - /mnt/archive/data:/data
    ports:
      - "9006:9000"
      - "9007:9001"
```

## API Design

### **Storage Tiering API**

```rust
use axum::{extract::Path, response::Json, routing::get, Router};
use serde_json::{json, Value};

pub fn storage_router() -> Router {
    Router::new()
        .route("/tiers", get(list_tiers))
        .route("/tiers/:tier/status", get(get_tier_status))
        .route("/tiers/:tier/migrate", post(migrate_to_tier))
        .route("/policies", get(get_policies))
        .route("/policies", post(update_policies))
        .route("/analytics", get(get_storage_analytics))
}

async fn list_tiers() -> Json<Value> {
    // Return list of available storage tiers
    Json(json!({
        "tiers": [
            {
                "name": "hot",
                "endpoint": "http://localhost:9000",
                "max_size_gb": 1000,
                "latency_threshold_ms": 1,
                "cost_per_gb": 0.10
            },
            {
                "name": "warm",
                "endpoint": "http://localhost:9002",
                "max_size_gb": 10000,
                "latency_threshold_ms": 10,
                "cost_per_gb": 0.05
            }
        ]
    }))
}

async fn get_tier_status(Path(tier): Path<String>) -> Json<Value> {
    // Return status of specific tier
    Json(json!({
        "tier": tier,
        "status": "healthy",
        "usage_gb": 500,
        "max_size_gb": 1000,
        "utilization_percent": 50.0
    }))
}

async fn migrate_to_tier(Path(tier): Path<String>) -> Json<Value> {
    // Manually migrate data to specific tier
    Json(json!({
        "message": format!("Migration to {} tier scheduled", tier),
        "status": "scheduled"
    }))
}

async fn get_policies() -> Json<Value> {
    // Return current tiering policies
    Json(json!({
        "automatic_tiering": true,
        "frequency_thresholds": {
            "hot_to_warm": 5.0,
            "warm_to_cold": 1.0,
            "cold_to_archive": 0.1
        },
        "time_based_tiering": {
            "hot_retention_days": 7,
            "warm_retention_days": 30,
            "cold_retention_days": 365
        }
    }))
}

async fn update_policies(Json(policies): Json<Value>) -> Json<Value> {
    // Update tiering policies
    Json(json!({
        "message": "Policies updated successfully",
        "status": "success"
    }))
}

async fn get_storage_analytics() -> Json<Value> {
    // Return storage analytics and usage patterns
    Json(json!({
        "total_storage_gb": 5000,
        "tier_usage": {
            "hot": {"usage_gb": 100, "files": 1000},
            "warm": {"usage_gb": 500, "files": 5000},
            "cold": {"usage_gb": 2000, "files": 20000},
            "archive": {"usage_gb": 2400, "files": 50000}
        },
        "access_patterns": {
            "hot_files": 1000,
            "warm_files": 5000,
            "cold_files": 20000,
            "archive_files": 50000
        }
    }))
}
```

## Monitoring and Observability

### **Prometheus Metrics**

```rust
use prometheus::{Counter, Histogram, Gauge, Registry};

pub struct StorageMetrics {
    pub tier_usage_gb: Gauge,
    pub tier_files_count: Gauge,
    pub tier_migrations_total: Counter,
    pub tier_migration_duration: Histogram,
    pub tier_access_latency: Histogram,
}

impl StorageMetrics {
    pub fn new(registry: &Registry) -> Self {
        let tier_usage_gb = Gauge::new(
            "storage_tier_usage_gb",
            "Storage usage in GB per tier"
        ).unwrap();
        
        let tier_files_count = Gauge::new(
            "storage_tier_files_count",
            "Number of files per tier"
        ).unwrap();
        
        let tier_migrations_total = Counter::new(
            "storage_tier_migrations_total",
            "Total number of tier migrations"
        ).unwrap();
        
        let tier_migration_duration = Histogram::new(
            "storage_tier_migration_duration_seconds",
            "Duration of tier migrations"
        ).unwrap();
        
        let tier_access_latency = Histogram::new(
            "storage_tier_access_latency_seconds",
            "Access latency per tier"
        ).unwrap();

        registry.register(Box::new(tier_usage_gb.clone())).unwrap();
        registry.register(Box::new(tier_files_count.clone())).unwrap();
        registry.register(Box::new(tier_migrations_total.clone())).unwrap();
        registry.register(Box::new(tier_migration_duration.clone())).unwrap();
        registry.register(Box::new(tier_access_latency.clone())).unwrap();

        Self {
            tier_usage_gb,
            tier_files_count,
            tier_migrations_total,
            tier_migration_duration,
            tier_access_latency,
        }
    }
}
```

### **Grafana Dashboard**

```json
{
  "dashboard": {
    "title": "Financial Data Service - Storage Tiering",
    "panels": [
      {
        "title": "Storage Usage by Tier",
        "type": "stat",
        "targets": [
          {
            "expr": "storage_tier_usage_gb{tier=\"hot\"}",
            "legendFormat": "Hot Tier (GB)"
          },
          {
            "expr": "storage_tier_usage_gb{tier=\"warm\"}",
            "legendFormat": "Warm Tier (GB)"
          },
          {
            "expr": "storage_tier_usage_gb{tier=\"cold\"}",
            "legendFormat": "Cold Tier (GB)"
          },
          {
            "expr": "storage_tier_usage_gb{tier=\"archive\"}",
            "legendFormat": "Archive Tier (GB)"
          }
        ]
      },
      {
        "title": "Tier Migration Rate",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(storage_tier_migrations_total[5m])",
            "legendFormat": "Migrations per second"
          }
        ]
      },
      {
        "title": "Access Latency by Tier",
        "type": "graph",
        "targets": [
          {
            "expr": "histogram_quantile(0.95, storage_tier_access_latency_seconds_bucket)",
            "legendFormat": "95th percentile latency"
          }
        ]
      }
    ]
  }
}
```

## Benefits

### **Performance Benefits**
- **Hot Tier (NVMe)**: < 1ms latency for frequently accessed data
- **Warm Tier (SSD)**: < 10ms latency for moderately accessed data
- **Cold Tier (HDD)**: < 100ms latency for rarely accessed data
- **Archive Tier (S3)**: < 1s latency for long-term storage

### **Cost Benefits**
- **Automatic Tiering**: Reduces storage costs by 60-80%
- **Usage-Based**: Data automatically moves to appropriate tier
- **Manual Override**: Administrators can control specific datasets
- **Cost Optimization**: Prevents over-provisioning of expensive storage

### **Operational Benefits**
- **Easy Configuration**: YAML-based configuration
- **Docker Integration**: Simple deployment with Docker Compose
- **Monitoring**: Comprehensive metrics and dashboards
- **API Management**: RESTful API for tier management

This design provides a comprehensive storage tiering solution using MinIO that can scale from development to production environments with configurable performance and cost optimization.
