# 🎯 EconGraph Financial Data Crate Split Plan

## 📊 Current State Analysis

### econ-graph-financial-data Crate Analysis

**Current Metrics:**
- **Total Files**: 37 files
- **Total Lines**: 11,962 LOC (but actual src/ is 3,573 LOC)
- **Size**: 648K
- **Functions**: 153
- **Structs**: 35
- **Enums**: 2
- **Implementations**: 34
- **Modules**: 15
- **Complexity Score**: 7.1 (MEDIUM-HIGH)

### File Size Breakdown

| File | Lines | Purpose |
|------|-------|---------|
| `storage/parquet_storage.rs` | 563 | Parquet storage implementation |
| `storage/iceberg_storage.rs` | 551 | Iceberg storage implementation |
| `catalog/file_catalog.rs` | 511 | File catalog management |
| `catalog/series_metadata.rs` | 347 | Series metadata handling |
| `monitoring/mod.rs` | 332 | Monitoring and metrics |
| `graphql/query.rs` | 332 | GraphQL query resolvers |
| `database/mod.rs` | 235 | Database operations |
| `catalog/mod.rs` | 139 | Catalog module exports |
| `main.rs` | 129 | Main application entry |
| `storage/in_memory_storage.rs` | 97 | In-memory storage |
| `graphql/mutation.rs` | 97 | GraphQL mutation resolvers |
| `models/mod.rs` | 78 | Data models |
| `crawler/mod.rs` | 51 | Data crawling |
| `storage/mod.rs` | 43 | Storage module exports |
| `graphql/mod.rs` | 31 | GraphQL module exports |
| `models/input.rs` | 30 | Input models |
| `lib.rs` | 7 | Library exports |

## 🎯 Proposed Split Strategy

Based on the analysis, I propose splitting `econ-graph-financial-data` into **3 focused crates**:

### 1. **econ-graph-arrow-storage** (~1,200 LOC)
**Purpose**: Arrow Flight + Parquet + Iceberg storage implementations
**Files to Move**:
- `storage/parquet_storage.rs` (563 LOC)
- `storage/iceberg_storage.rs` (551 LOC)
- `storage/in_memory_storage.rs` (97 LOC)
- `storage/mod.rs` (43 LOC)

**Dependencies**:
- `arrow = "56.0"`
- `arrow-flight = "56.0"`
- `parquet = "56.0"`
- `object_store = "0.12"`
- `apache-avro = "0.14"`

### 2. **econ-graph-financial-graphql** (~500 LOC)
**Purpose**: GraphQL API layer for financial data
**Files to Move**:
- `graphql/query.rs` (332 LOC)
- `graphql/mutation.rs` (97 LOC)
- `graphql/mod.rs` (31 LOC)

**Dependencies**:
- `async-graphql = { workspace = true }`
- `async-graphql-axum = "7.0"`
- `econ-graph-arrow-storage` (new dependency)
- `econ-graph-financial-core` (new dependency)

### 3. **econ-graph-financial-core** (~1,873 LOC)
**Purpose**: Core financial data models, catalog, database, and monitoring
**Files to Move**:
- `catalog/file_catalog.rs` (511 LOC)
- `catalog/series_metadata.rs` (347 LOC)
- `monitoring/mod.rs` (332 LOC)
- `database/mod.rs` (235 LOC)
- `catalog/mod.rs` (139 LOC)
- `main.rs` (129 LOC)
- `models/mod.rs` (78 LOC)
- `crawler/mod.rs` (51 LOC)
- `models/input.rs` (30 LOC)
- `lib.rs` (7 LOC)

**Dependencies**:
- `econ-graph-core` (existing)
- `serde = { workspace = true }`
- `uuid = { workspace = true }`
- `chrono = { workspace = true }`
- `rust_decimal = { workspace = true }`

## 🏗️ Implementation Plan

### Phase 1: Create New Crate Structure

#### Step 1.1: Create New Crate Directories
```bash
# Create new crate directories
mkdir -p backend/crates/econ-graph-arrow-storage/src
mkdir -p backend/crates/econ-graph-financial-graphql/src
mkdir -p backend/crates/econ-graph-financial-core/src

# Create subdirectories for each crate
mkdir -p backend/crates/econ-graph-arrow-storage/src/storage
mkdir -p backend/crates/econ-graph-financial-graphql/src/graphql
mkdir -p backend/crates/econ-graph-financial-core/src/{catalog,models,database,monitoring,crawler}
```

#### Step 1.2: Create Cargo.toml Files

**econ-graph-arrow-storage/Cargo.toml**:
```toml
[package]
name = "econ-graph-arrow-storage"
version = "0.1.0"
edition = "2021"
authors = ["EconGraph Team"]
description = "Arrow Flight + Parquet + Iceberg storage implementations"
license = "MS-PL"

[dependencies]
# Arrow and Parquet
arrow = "56.0"
arrow-flight = "56.0"
parquet = "56.0"
object_store = "0.12"
apache-avro = "0.14"

# Serialization
serde = { workspace = true }
serde_json = { workspace = true }

# UUID and Time
uuid = { workspace = true }
chrono = { workspace = true }

# Decimal for financial data
rust_decimal = { workspace = true }

# Error handling
anyhow = { workspace = true }
thiserror = { workspace = true }

# Async traits
async-trait = "0.1"

# Logging
tracing = { workspace = true }
```

**econ-graph-financial-graphql/Cargo.toml**:
```toml
[package]
name = "econ-graph-financial-graphql"
version = "0.1.0"
edition = "2021"
authors = ["EconGraph Team"]
description = "GraphQL API layer for financial data"
license = "MS-PL"

[dependencies]
# GraphQL and Web Framework
async-graphql = { workspace = true }
async-graphql-axum = "7.0"

# Core dependencies
econ-graph-core = { workspace = true }
econ-graph-arrow-storage = { workspace = true }
econ-graph-financial-core = { workspace = true }

# Serialization
serde = { workspace = true }
serde_json = { workspace = true }

# UUID and Time
uuid = { workspace = true }
chrono = { workspace = true }

# Decimal for financial data
rust_decimal = { workspace = true }

# Error handling
anyhow = { workspace = true }
thiserror = { workspace = true }

# Logging
tracing = { workspace = true }
```

**econ-graph-financial-core/Cargo.toml**:
```toml
[package]
name = "econ-graph-financial-core"
version = "0.1.0"
edition = "2021"
authors = ["EconGraph Team"]
description = "Core financial data models, catalog, database, and monitoring"
license = "MS-PL"

[dependencies]
# Core dependencies
econ-graph-core = { workspace = true }

# Serialization
serde = { workspace = true }
serde_json = { workspace = true }

# UUID and Time
uuid = { workspace = true }
chrono = { workspace = true }

# Decimal for financial data
rust_decimal = { workspace = true }

# Configuration
config = { workspace = true }
clap = { workspace = true }

# Logging
tracing = { workspace = true }

# Error handling
anyhow = { workspace = true }
thiserror = { workspace = true }

# Async traits
async-trait = "0.1"

# Testing
[dev-dependencies]
tokio-test = { workspace = true }
tempfile = { workspace = true }
```

### Phase 2: Move Files Systematically

#### Step 2.1: Move Storage Files
```bash
# Move storage files to arrow-storage crate
mv backend/crates/econ-graph-financial-data/src/storage/parquet_storage.rs backend/crates/econ-graph-arrow-storage/src/storage/
mv backend/crates/econ-graph-financial-data/src/storage/iceberg_storage.rs backend/crates/econ-graph-arrow-storage/src/storage/
mv backend/crates/econ-graph-financial-data/src/storage/in_memory_storage.rs backend/crates/econ-graph-arrow-storage/src/storage/
mv backend/crates/econ-graph-financial-data/src/storage/mod.rs backend/crates/econ-graph-arrow-storage/src/storage/
```

#### Step 2.2: Move GraphQL Files
```bash
# Move GraphQL files to financial-graphql crate
mv backend/crates/econ-graph-financial-data/src/graphql/query.rs backend/crates/econ-graph-financial-graphql/src/graphql/
mv backend/crates/econ-graph-financial-data/src/graphql/mutation.rs backend/crates/econ-graph-financial-graphql/src/graphql/
mv backend/crates/econ-graph-financial-data/src/graphql/mod.rs backend/crates/econ-graph-financial-graphql/src/graphql/
```

#### Step 2.3: Move Core Files
```bash
# Move remaining files to financial-core crate
mv backend/crates/econ-graph-financial-data/src/catalog/ backend/crates/econ-graph-financial-core/src/
mv backend/crates/econ-graph-financial-data/src/models/ backend/crates/econ-graph-financial-core/src/
mv backend/crates/econ-graph-financial-data/src/database/ backend/crates/econ-graph-financial-core/src/
mv backend/crates/econ-graph-financial-data/src/monitoring/ backend/crates/econ-graph-financial-core/src/
mv backend/crates/econ-graph-financial-data/src/crawler/ backend/crates/econ-graph-financial-core/src/
mv backend/crates/econ-graph-financial-data/src/main.rs backend/crates/econ-graph-financial-core/src/
mv backend/crates/econ-graph-financial-data/src/lib.rs backend/crates/econ-graph-financial-core/src/
```

### Phase 3: Update Dependencies and Imports

#### Step 3.1: Update Workspace Cargo.toml
```toml
# Add to backend/Cargo.toml workspace members
[workspace]
members = [
    "crates/econ-graph-core",
    "crates/econ-graph-services", 
    "crates/econ-graph-auth",
    "crates/econ-graph-graphql",
    "crates/econ-graph-mcp",
    "crates/econ-graph-crawler",
    "crates/econ-graph-backend",
    "crates/econ-graph-metrics",
    "crates/econ-graph-sec-crawler",
    # New financial data crates
    "crates/econ-graph-arrow-storage",
    "crates/econ-graph-financial-graphql", 
    "crates/econ-graph-financial-core",
]
```

#### Step 3.2: Update Import Statements
- Update all imports in moved files to use new crate paths
- Update any external crates that depend on the old financial-data crate
- Ensure proper dependency chains between new crates

### Phase 4: Testing and Validation

#### Step 4.1: Compilation Testing
```bash
# Test each new crate compiles independently
cd backend/crates/econ-graph-arrow-storage && cargo check
cd backend/crates/econ-graph-financial-graphql && cargo check  
cd backend/crates/econ-graph-financial-core && cargo check

# Test workspace compilation
cd backend && cargo check
```

#### Step 4.2: Integration Testing
- Run existing tests to ensure functionality is preserved
- Test cross-crate dependencies work correctly
- Verify GraphQL API still functions properly

## 📈 Expected Benefits

### **Compilation Performance**
- **Arrow Storage**: ~1,200 LOC → Faster compilation for storage changes
- **GraphQL API**: ~500 LOC → Quick iteration on API changes  
- **Financial Core**: ~1,873 LOC → Focused core logic compilation

### **Development Experience**
- **Clear Separation**: Storage, API, and core logic are distinct
- **Focused Testing**: Each crate can be tested independently
- **Better IDE Performance**: Smaller crates load faster in IDEs

### **Maintainability**
- **Single Responsibility**: Each crate has a clear purpose
- **Reduced Coupling**: Dependencies are explicit and minimal
- **Easier Refactoring**: Changes isolated to specific domains

## 🎯 Final Crate Structure

| Crate | LOC | Purpose | Key Dependencies |
|-------|-----|---------|------------------|
| **econ-graph-arrow-storage** | ~1,200 | Arrow Flight + Parquet + Iceberg | Arrow, Parquet, Object Store |
| **econ-graph-financial-graphql** | ~500 | GraphQL API layer | Async-GraphQL, Arrow Storage |
| **econ-graph-financial-core** | ~1,873 | Core models, catalog, monitoring | Econ-Graph-Core |

## 🚨 Risk Mitigation

### **Compilation Risks**
- Test each step incrementally
- Maintain working state after each phase
- Have rollback plan ready

### **Integration Risks**
- Thoroughly test cross-crate dependencies
- Verify all imports work correctly
- Test end-to-end functionality

### **Dependency Risks**
- Ensure proper dependency chains
- Avoid circular dependencies
- Test with existing dependent crates

## 📅 Timeline

| Phase | Duration | Tasks |
|-------|----------|-------|
| **Phase 1** | 1 day | Create crate structure and Cargo.toml files |
| **Phase 2** | 1 day | Move files to new crates |
| **Phase 3** | 1 day | Update dependencies and imports |
| **Phase 4** | 1 day | Testing and validation |

**Total Duration**: 4 days
**Expected Complexity Reduction**: 60% (from 7.1 to ~3.0 average)
**Expected Build Time Improvement**: 40%+

---

*This focused plan addresses the immediate need to split the financial data crate while preserving the comprehensive plan for future crate splits.*
