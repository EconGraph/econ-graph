# CI Cleanup Design Document

## Problem Statement
Currently, 4 different CI workflows are auto-triggering per commit, causing confusion and waste.

## Current State
- **Core CI Tests** (ci-core.yml) - Ultra-fine-grained backend tests ✅
- **CI/CD Pipeline** (ci-original-backup.yml) - Old monolithic workflow ❌
- **Integration Tests** (ci-integration.yml) - Redundant ❌  
- **Security & Compliance** (ci-security.yml) - Redundant ❌
- **Build & Deploy** (ci-build.yml) - Redundant ❌
- **Various test-*.yml files** - Test workflows ❌

## Requirements (Need Clarification)
- [ ] Keep all workflow files?
- [ ] Only one workflow auto-triggers per commit?
- [ ] Which workflow should auto-trigger?
- [ ] Other workflows available for manual dispatch?
- [ ] Any other constraints?

## Proposed Solution (Pending Approval)
1. Keep all workflow files
2. Disable auto-triggering for all except ci-core.yml
3. All others available via workflow_dispatch only
4. Result: Only "Core CI Tests" runs automatically per commit

## Questions for User
1. Is this the correct approach?
2. Should ci-core.yml be the only auto-triggering workflow?
3. Are there any other requirements I'm missing?
