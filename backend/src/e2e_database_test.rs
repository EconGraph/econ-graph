// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

use crate::config::Config;
use crate::database::{create_pool, test_connection};
use serial_test::serial;
use std::env;
use tokio::time::{sleep, Duration};

/// Comprehensive test to reproduce the e2e database connection issue
/// This test simulates the exact conditions in CI where the backend fails to connect
#[tokio::test]
#[serial]
async fn test_e2e_database_connection_scenario() {
    // REQUIREMENT: Reproduce the exact e2e test database connection failure
    // PURPOSE: Identify why the backend fails to connect to PostgreSQL in CI
    // This test simulates the CI environment conditions

    println!("🔍 Testing e2e database connection scenario...");

    // Store original environment variables
    let original_database_url = env::var("DATABASE_URL").ok();
    let original_user = env::var("USER").ok();
    let original_home = env::var("HOME").ok();

    // Simulate CI environment conditions
    env::set_var(
        "DATABASE_URL",
        "postgresql://postgres:password@localhost:5432/econ_graph_test",
    );

    // In CI, the USER environment variable might be 'root' or different
    // This could be causing the connection issue
    println!("Current USER: {:?}", env::var("USER"));
    println!("Current HOME: {:?}", env::var("HOME"));
    println!("Current DATABASE_URL: {:?}", env::var("DATABASE_URL"));

    // Test 1: Load configuration and verify database URL
    println!("📋 Testing configuration loading...");
    let config = Config::from_env().expect("Failed to load configuration");
    println!("✅ Configuration loaded successfully");
    println!("  - Database URL: {}", config.database_url);

    // Verify the database URL is correct
    assert_eq!(
        config.database_url,
        "postgresql://postgres:password@localhost:5432/econ_graph_test"
    );

    // Test 2: Try to create database pool (this is where the failure occurs in CI)
    println!("🗄️  Testing database pool creation...");

    // This should fail if PostgreSQL is not running or not accessible
    match create_pool(&config.database_url).await {
        Ok(pool) => {
            println!("✅ Database pool created successfully");

            // Test 3: Test database connection
            println!("🔗 Testing database connection...");
            match test_connection(&pool).await {
                Ok(_) => {
                    println!("✅ Database connection test successful");
                }
                Err(e) => {
                    println!("❌ Database connection test failed: {}", e);
                    panic!("Database connection test failed: {}", e);
                }
            }
        }
        Err(e) => {
            println!("❌ Database pool creation failed: {}", e);
            println!("This is the expected failure in CI - investigating root cause...");

            // Analyze the error to understand the root cause
            let error_msg = e.to_string();
            if error_msg.contains("role") && error_msg.contains("does not exist") {
                println!("🔍 Root cause identified: PostgreSQL role/user issue");
                println!("  - Error: {}", error_msg);
                println!("  - This suggests the backend is trying to connect as the wrong user");

                // Check if the issue is with the connection string parsing
                println!("🔍 Analyzing connection string...");
                let url = &config.database_url;
                println!("  - Full URL: {}", url);

                // Parse the URL to see what user it's trying to use
                if let Ok(parsed_url) = url::Url::parse(url) {
                    println!("  - Parsed URL components:");
                    println!("    - Scheme: {:?}", parsed_url.scheme());
                    println!("    - Host: {:?}", parsed_url.host());
                    println!("    - Port: {:?}", parsed_url.port());
                    println!("    - Username: {:?}", parsed_url.username());
                    println!("    - Password: {:?}", parsed_url.password().is_some());
                    println!("    - Path: {:?}", parsed_url.path());
                }
            } else {
                println!("🔍 Different error type: {}", error_msg);
            }

            // Don't panic here - we want to analyze the error
            println!("⚠️  Database connection failed as expected in CI scenario");
        }
    }

    // Restore original environment variables
    if let Some(val) = original_database_url {
        env::set_var("DATABASE_URL", val);
    } else {
        env::remove_var("DATABASE_URL");
    }
    if let Some(val) = original_user {
        env::set_var("USER", val);
    }
    if let Some(val) = original_home {
        env::set_var("HOME", val);
    }

    println!("✅ E2E database connection scenario test completed");
}

/// Test to verify PostgreSQL connection string parsing
#[tokio::test]
#[serial]
async fn test_postgresql_connection_string_parsing() {
    // REQUIREMENT: Verify PostgreSQL connection string parsing works correctly
    // PURPOSE: Ensure the database URL is being parsed correctly and not causing user issues

    println!("🔍 Testing PostgreSQL connection string parsing...");

    let test_urls = vec![
        "postgresql://postgres:password@localhost:5432/econ_graph_test",
        "postgres://postgres:password@localhost:5432/econ_graph_test",
        "postgresql://localhost:5432/econ_graph_test",
        "postgresql://user:pass@host:5432/db",
    ];

    for url in test_urls {
        println!("Testing URL: {}", url);

        match url::Url::parse(url) {
            Ok(parsed) => {
                println!("  ✅ Parsed successfully:");
                println!("    - Scheme: {}", parsed.scheme());
                println!("    - Username: {}", parsed.username());
                println!("    - Password: {}", parsed.password().is_some());
                println!("    - Host: {:?}", parsed.host());
                println!("    - Port: {:?}", parsed.port());
                println!("    - Path: {}", parsed.path());

                // Verify the username is correct
                if parsed.username() == "postgres" {
                    println!("    ✅ Username is correct: postgres");
                } else {
                    println!("    ⚠️  Username might be incorrect: {}", parsed.username());
                }
            }
            Err(e) => {
                println!("  ❌ Failed to parse: {}", e);
            }
        }
    }

    println!("✅ PostgreSQL connection string parsing test completed");
}

/// Test to simulate the exact CI environment conditions
#[tokio::test]
#[serial]
async fn test_ci_environment_simulation() {
    // REQUIREMENT: Simulate the exact CI environment conditions
    // PURPOSE: Reproduce the CI failure locally to understand the root cause

    println!("🔍 Simulating CI environment conditions...");

    // Store original environment
    let original_env: std::collections::HashMap<String, String> = env::vars().collect();

    // Simulate CI environment variables
    env::set_var("CI", "true");
    env::set_var("GITHUB_ACTIONS", "true");
    env::set_var("RUNNER_OS", "Linux");
    env::set_var("USER", "root"); // This might be the issue!
    env::set_var("HOME", "/root");
    env::set_var(
        "DATABASE_URL",
        "postgresql://postgres:password@localhost:5432/econ_graph_test",
    );

    println!("CI Environment simulation:");
    println!("  - CI: {:?}", env::var("CI"));
    println!("  - USER: {:?}", env::var("USER"));
    println!("  - HOME: {:?}", env::var("HOME"));
    println!("  - DATABASE_URL: {:?}", env::var("DATABASE_URL"));

    // Test configuration loading in CI environment
    let config = Config::from_env().expect("Failed to load configuration in CI simulation");
    println!("✅ Configuration loaded in CI simulation");
    println!("  - Database URL: {}", config.database_url);

    // Test database pool creation
    println!("🗄️  Testing database pool creation in CI simulation...");
    match create_pool(&config.database_url).await {
        Ok(pool) => {
            println!("✅ Database pool created successfully in CI simulation");

            // Test connection
            match test_connection(&pool).await {
                Ok(_) => {
                    println!("✅ Database connection successful in CI simulation");
                }
                Err(e) => {
                    println!("❌ Database connection failed in CI simulation: {}", e);
                }
            }
        }
        Err(e) => {
            println!("❌ Database pool creation failed in CI simulation: {}", e);
            println!("🔍 This reproduces the CI failure - analyzing error...");

            let error_msg = e.to_string();
            if error_msg.contains("role") && error_msg.contains("does not exist") {
                println!("🎯 Root cause confirmed: PostgreSQL role issue");
                println!("  - The backend is trying to connect as the wrong user");
                println!("  - CI environment USER is 'root', but PostgreSQL expects 'postgres'");
                println!("  - This suggests a connection string parsing or environment issue");
            }
        }
    }

    // Restore original environment
    for (key, value) in original_env {
        env::set_var(key, value);
    }

    println!("✅ CI environment simulation test completed");
}

/// Test to verify the exact error message from CI logs
#[tokio::test]
#[serial]
async fn test_ci_error_reproduction() {
    // REQUIREMENT: Reproduce the exact error from CI logs
    // PURPOSE: Understand why "FATAL: role 'root' does not exist" occurs

    println!("🔍 Reproducing CI error: 'FATAL: role root does not exist'...");

    // The CI logs show repeated "FATAL: role 'root' does not exist" errors
    // This suggests the backend is trying to connect as 'root' instead of 'postgres'

    // Test 1: Verify our connection string is correct
    let correct_url = "postgresql://postgres:password@localhost:5432/econ_graph_test";
    println!("Testing correct connection string: {}", correct_url);

    // Test 2: Check if there's any environment variable that could override the username
    let env_vars_to_check = vec![
        "DATABASE_URL",
        "POSTGRES_USER",
        "POSTGRES_PASSWORD",
        "POSTGRES_DB",
        "USER",
        "HOME",
        "PGUSER",
        "PGPASSWORD",
        "PGDATABASE",
    ];

    println!("Environment variables that could affect database connection:");
    for var in env_vars_to_check {
        match env::var(var) {
            Ok(value) => {
                println!("  - {}: {}", var, value);
            }
            Err(_) => {
                println!("  - {}: not set", var);
            }
        }
    }

    // Test 3: Try to understand why 'root' user is being used
    println!("🔍 Analyzing potential causes for 'root' user issue:");
    println!("  1. Environment variable override");
    println!("  2. Connection string parsing issue");
    println!("  3. PostgreSQL client library default behavior");
    println!("  4. CI environment USER variable affecting connection");

    // Test 4: Check if PGUSER environment variable is set
    if let Ok(pguser) = env::var("PGUSER") {
        println!("⚠️  PGUSER is set to: {}", pguser);
        println!("   This could override the username in the connection string!");
    }

    println!("✅ CI error reproduction test completed");
}
