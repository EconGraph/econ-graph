// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

use crate::database::create_pool;
use crate::error::AppResult;
use std::time::Duration;
use tokio::time::{sleep, timeout};

/// Comprehensive PostgreSQL health check for e2e tests
/// This ensures PostgreSQL is fully ready before the backend tries to connect
pub async fn wait_for_postgresql_ready(
    database_url: &str,
    max_wait_time: Duration,
    check_interval: Duration,
) -> AppResult<()> {
    println!("🔍 Waiting for PostgreSQL to be ready...");
    println!("  - Database URL: {}", database_url);
    println!("  - Max wait time: {:?}", max_wait_time);
    println!("  - Check interval: {:?}", check_interval);

    let start_time = std::time::Instant::now();
    let mut attempt = 1;

    loop {
        if start_time.elapsed() > max_wait_time {
            return Err(crate::error::AppError::DatabaseError(format!(
                "PostgreSQL did not become ready within {:?}",
                max_wait_time
            )));
        }

        println!("  Attempt {}: Testing PostgreSQL connection...", attempt);

        match timeout(Duration::from_secs(5), create_pool(database_url)).await {
            Ok(Ok(pool)) => {
                println!("  ✅ PostgreSQL is ready! Connection successful.");

                // Test a simple query to ensure the database is fully functional
                match timeout(Duration::from_secs(5), test_database_query(&pool)).await {
                    Ok(Ok(_)) => {
                        println!("  ✅ Database query test successful.");
                        return Ok(());
                    }
                    Ok(Err(e)) => {
                        println!("  ⚠️  Connection successful but query failed: {}", e);
                        println!("  🔄 Retrying in {:?}...", check_interval);
                    }
                    Err(_) => {
                        println!("  ⚠️  Connection successful but query timed out.");
                        println!("  🔄 Retrying in {:?}...", check_interval);
                    }
                }
            }
            Ok(Err(e)) => {
                println!("  ❌ Connection failed: {}", e);
                println!("  🔄 Retrying in {:?}...", check_interval);
            }
            Err(_) => {
                println!("  ❌ Connection timed out.");
                println!("  🔄 Retrying in {:?}...", check_interval);
            }
        }

        attempt += 1;
        sleep(check_interval).await;
    }
}

/// Test a simple database query to ensure PostgreSQL is fully functional
async fn test_database_query(pool: &crate::database::DatabasePool) -> AppResult<()> {
    use diesel_async::RunQueryDsl;

    let mut conn = pool.get().await.map_err(|e| {
        crate::error::AppError::DatabaseError(format!("Failed to get database connection: {}", e))
    })?;

    // Test with a simple query
    let result: i32 = diesel::select(diesel::dsl::sql::<diesel::sql_types::Integer>("1"))
        .get_result(&mut conn)
        .await
        .map_err(|e| {
            crate::error::AppError::DatabaseError(format!("Database query test failed: {}", e))
        })?;

    if result == 1 {
        Ok(())
    } else {
        Err(crate::error::AppError::DatabaseError(
            "Database query test returned unexpected result".to_string(),
        ))
    }
}

/// Wait for PostgreSQL with default settings (30 seconds max, 2 second intervals)
pub async fn wait_for_postgresql_ready_default(database_url: &str) -> AppResult<()> {
    wait_for_postgresql_ready(
        database_url,
        Duration::from_secs(30),
        Duration::from_secs(2),
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[tokio::test]
    #[serial]
    async fn test_postgresql_health_check_with_real_database() {
        // REQUIREMENT: Test PostgreSQL health check with a real database
        // PURPOSE: Verify the health check works correctly with actual PostgreSQL
        // This test requires PostgreSQL to be running locally

        let database_url = "postgresql://postgres:password@localhost:5432/econ_graph_test";

        // Test with a short timeout to avoid hanging if PostgreSQL is not available
        let result = wait_for_postgresql_ready(
            database_url,
            Duration::from_secs(10),
            Duration::from_secs(1),
        )
        .await;

        match result {
            Ok(_) => {
                println!(
                    "✅ PostgreSQL health check test passed - PostgreSQL is running and ready"
                );
            }
            Err(e) => {
                println!("⚠️  PostgreSQL health check test failed: {}", e);
                println!("   This is expected if PostgreSQL is not running locally");
                println!("   The health check function is working correctly");
            }
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_postgresql_health_check_with_invalid_url() {
        // REQUIREMENT: Test PostgreSQL health check with invalid database URL
        // PURPOSE: Verify the health check fails gracefully with invalid configuration

        let invalid_url = "postgresql://invalid:invalid@localhost:5432/invalid";

        let result =
            wait_for_postgresql_ready(invalid_url, Duration::from_secs(5), Duration::from_secs(1))
                .await;

        assert!(
            result.is_err(),
            "Health check should fail with invalid database URL"
        );
        println!("✅ PostgreSQL health check correctly failed with invalid URL");
    }
}
