// Copyright (c) 2024 EconGraph. All rights reserved.
// Licensed under the Microsoft Reference Source License (MS-RSL).
// See LICENSE file for complete terms and conditions.

use diesel::prelude::*;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use imara_diff::{Algorithm, UnifiedDiffBuilder};
use std::fs;
use std::process::Command;
use testcontainers::{runners::AsyncRunner, ContainerAsync};
use testcontainers_modules::postgres::Postgres;

// Embed migrations at compile time
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations/");

/// Test that validates schema compatibility between migrations and schema.rs
///
/// This test:
/// 1. Starts a fresh PostgreSQL container using testcontainers
/// 2. Applies all migrations to create the database schema
/// 3. Generates a new schema.rs using diesel print-schema
/// 4. Compares the generated schema with the existing schema.rs
/// 5. Fails if there are any differences, indicating schema drift
#[tokio::test]
async fn test_schema_compatibility() {
    // Start PostgreSQL container
    let postgres_image = Postgres::default()
        .with_db_name("test_econ_graph")
        .with_user("test_user")
        .with_password("test_password");
    let container: ContainerAsync<Postgres> = postgres_image
        .start()
        .await
        .expect("Failed to start container");

    // Get connection details
    let host_port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("Failed to get port");
    let database_url = format!(
        "postgres://test_user:test_password@localhost:{}/test_econ_graph",
        host_port
    );

    // Wait a moment for the database to be ready
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Establish connection
    let mut conn =
        PgConnection::establish(&database_url).expect("Failed to connect to test database");

    // Apply all migrations
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run migrations");

    // Generate schema using diesel print-schema
    let generated_schema =
        generate_schema_with_diesel(&database_url).expect("Failed to generate schema with diesel");

    // Read existing schema.rs
    let existing_schema =
        fs::read_to_string("src/schema.rs").expect("Failed to read existing schema.rs");

    // Normalize both schemas for comparison
    let normalized_generated = normalize_schema(&generated_schema);
    let normalized_existing = normalize_schema(&existing_schema);

    // Compare schemas
    if normalized_generated != normalized_existing {
        println!("=== SCHEMA MISMATCH DETECTED ===");
        println!("Generated schema length: {}", generated_schema.len());
        println!("Existing schema length: {}", existing_schema.len());

        // Write generated schema to a file for debugging
        fs::write("/tmp/generated_schema.rs", &generated_schema)
            .expect("Failed to write generated schema for debugging");
        println!("Generated schema written to /tmp/generated_schema.rs for inspection");

        // Find and display differences
        let differences = find_schema_differences(&normalized_generated, &normalized_existing);
        for diff in differences {
            println!("Difference: {}", diff);
        }

        panic!("Schema mismatch detected! Generated schema differs from existing schema.rs");
    }

    println!("✅ Schema compatibility test passed - schema.rs matches database schema");
}

/// Generate schema using diesel print-schema command
fn generate_schema_with_diesel(database_url: &str) -> Result<String, Box<dyn std::error::Error>> {
    // Try to find diesel in common locations
    let diesel_paths = [
        "diesel",
        "/Users/josephmalicki/.cargo/bin/diesel",
        "/usr/local/bin/diesel",
        "/usr/bin/diesel",
    ];

    let mut diesel_cmd = None;
    for path in &diesel_paths {
        if Command::new("which").arg(path).output().is_ok() {
            diesel_cmd = Some(path);
            break;
        }
    }

    let diesel_path = diesel_cmd.ok_or("diesel CLI not found in PATH or common locations")?;

    println!("Using diesel at: {}", diesel_path);
    println!("Database URL: {}", database_url);

    let output = Command::new(diesel_path)
        .args(["print-schema", "--database-url", database_url])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "diesel print-schema failed with status {:?}\nSTDERR: {}\nSTDOUT: {}",
            output.status, stderr, stdout
        )
        .into());
    }

    let schema = String::from_utf8(output.stdout)?;
    Ok(schema)
}

/// Normalize schema content for comparison
///
/// This function:
/// 1. Removes comments and whitespace differences
/// 2. Sorts table definitions consistently
/// 3. Normalizes whitespace and formatting
fn normalize_schema(schema: &str) -> String {
    let mut lines: Vec<&str> = schema.lines().collect();

    // Remove empty lines and comments
    lines.retain(|line| {
        let trimmed = line.trim();
        !trimmed.is_empty() && !trimmed.starts_with("//")
    });

    // Sort lines to ensure consistent ordering
    lines.sort();

    // Join lines with single newlines
    lines.join("\n")
}

/// Find specific differences between two schemas
fn find_schema_differences(schema1: &str, schema2: &str) -> Vec<String> {
    let lines1: Vec<&str> = schema1.lines().collect();
    let lines2: Vec<&str> = schema2.lines().collect();

    let mut differences = Vec::new();

    // Find lines that are in schema1 but not in schema2
    for line in &lines1 {
        if !lines2.contains(line) {
            differences.push(format!("Missing in existing: {}", line));
        }
    }

    // Find lines that are in schema2 but not in schema1
    for line in &lines2 {
        if !lines1.contains(line) {
            differences.push(format!("Extra in existing: {}", line));
        }
    }

    differences
}

/// Test that validates the schema generation process works correctly
#[tokio::test]
async fn test_schema_generation_process() {
    // Start PostgreSQL container
    let postgres_image = Postgres::default()
        .with_db_name("test_econ_graph")
        .with_user("test_user")
        .with_password("test_password");
    let container: ContainerAsync<Postgres> = postgres_image
        .start()
        .await
        .expect("Failed to start container");

    // Get connection details
    let host_port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("Failed to get port");
    let database_url = format!(
        "postgres://test_user:test_password@localhost:{}/test_econ_graph",
        host_port
    );

    // Wait for database to be ready
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Apply migrations first
    let mut conn =
        PgConnection::establish(&database_url).expect("Failed to connect to test database");
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run migrations");

    // Test that we can connect and generate schema
    let generated_schema = generate_schema_with_diesel(&database_url);

    match generated_schema {
        Ok(schema) => {
            println!(
                "Generated schema (first 500 chars): {}",
                &schema[..schema.len().min(500)]
            );
            assert!(!schema.is_empty(), "Generated schema should not be empty");
            assert!(
                schema.contains("diesel::table!"),
                "Generated schema should contain diesel table definitions"
            );
            println!(
                "✅ Schema generation test passed - generated {} characters",
                schema.len()
            );
        }
        Err(e) => {
            panic!("Schema generation failed: {}", e);
        }
    }
}

/// Test that validates migration application works correctly
#[tokio::test]
async fn test_migration_application() {
    // Start PostgreSQL container
    let postgres_image = Postgres::default()
        .with_db_name("test_econ_graph")
        .with_user("test_user")
        .with_password("test_password");
    let container: ContainerAsync<Postgres> = postgres_image
        .start()
        .await
        .expect("Failed to start container");

    // Get connection details
    let host_port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("Failed to get port");
    let database_url = format!(
        "postgres://test_user:test_password@localhost:{}/test_econ_graph",
        host_port
    );

    // Wait for database to be ready
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Establish connection
    let mut conn =
        PgConnection::establish(&database_url).expect("Failed to connect to test database");

    // Apply migrations
    let result = conn.run_pending_migrations(MIGRATIONS);

    match result {
        Ok(_) => {
            println!("✅ Migration application test passed - all migrations applied successfully");
        }
        Err(e) => {
            panic!("Migration application failed: {}", e);
        }
    }
}

/// Test that compares generated schema with existing schema.rs
#[tokio::test]
async fn test_schema_compatibility_comparison() {
    // Start PostgreSQL container
    let postgres_image = Postgres::default()
        .with_db_name("test_econ_graph")
        .with_user("test_user")
        .with_password("test_password");
    let container: ContainerAsync<Postgres> = postgres_image
        .start()
        .await
        .expect("Failed to start container");

    // Get connection details
    let host_port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("Failed to get port");
    let database_url = format!(
        "postgres://test_user:test_password@localhost:{}/test_econ_graph",
        host_port
    );

    // Wait for database to be ready
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    // Apply migrations first
    let mut conn =
        PgConnection::establish(&database_url).expect("Failed to connect to test database");
    conn.run_pending_migrations(MIGRATIONS)
        .expect("Failed to run migrations");

    // Generate schema from the migrated database
    let generated_schema =
        generate_schema_with_diesel(&database_url).expect("Failed to generate schema");

    // Read the existing schema.rs file
    let existing_schema =
        fs::read_to_string("src/schema.rs").expect("Failed to read existing schema.rs");

    // Normalize both schemas for comparison
    let normalized_generated = normalize_schema(&generated_schema);
    let normalized_existing = normalize_schema(&existing_schema);

    // Compare the normalized schemas
    if normalized_generated != normalized_existing {
        println!("❌ Schema mismatch detected!");
        println!("Generated schema length: {}", normalized_generated.len());
        println!("Existing schema length: {}", normalized_existing.len());

        // Generate and print unified diff
        let diff_output = generate_schema_diff(&generated_schema, &existing_schema);
        println!("{}", diff_output);

        panic!(
            "Schema compatibility test failed - generated schema does not match existing schema.rs"
        );
    }

    println!("✅ Schema compatibility test passed - schemas match");
}

/// Generate a proper unified diff between two schemas using imara-diff
fn generate_schema_diff(generated_schema: &str, existing_schema: &str) -> String {
    // Use imara-diff to generate a proper unified diff
    let input = imara_diff::intern::InternedInput::new(generated_schema, existing_schema);

    let builder = UnifiedDiffBuilder::new(&input);

    let result = imara_diff::diff(Algorithm::Histogram, &input, builder);

    // If we got diff output, use it
    if !result.is_empty() {
        result
    } else {
        "✅ No differences found between schemas.\n".to_string()
    }
}
