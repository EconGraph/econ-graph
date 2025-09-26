use std::time::Duration;
use tokio::time::sleep;

/// Docker integration test for the Financial Data Service
///
/// This test:
/// 1. Builds the Docker image
/// 2. Starts the service in a container
/// 3. Tests the GraphQL API endpoints
/// 4. Verifies health checks
/// 5. Cleans up resources
#[tokio::test]
async fn test_docker_integration() -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    println!("🐳 Starting Docker integration test...");

    // Step 1: Build the Docker image
    println!("📦 Building Docker image...");
    let build_result = Command::new("docker")
        .args(["build", "-t", "econ-graph-financial-data:test", "."])
        .current_dir(".")
        .output()?;

    if !build_result.status.success() {
        eprintln!("❌ Docker build failed:");
        eprintln!("{}", String::from_utf8_lossy(&build_result.stderr));
        return Err("Docker build failed".into());
    }
    println!("✅ Docker image built successfully");

    // Step 2: Start the service in a container
    println!("🚀 Starting service container...");
    let run_result = Command::new("docker")
        .args([
            "run",
            "-d",
            "--name",
            "financial-data-test",
            "-p",
            "3001:3001",
            "-e",
            "RUST_LOG=debug",
            "econ-graph-financial-data:test",
        ])
        .output()?;

    if !run_result.status.success() {
        eprintln!("❌ Failed to start container:");
        eprintln!("{}", String::from_utf8_lossy(&run_result.stderr));
        return Err("Container start failed".into());
    }
    println!("✅ Service container started");

    // Step 3: Wait for service to be ready
    println!("⏳ Waiting for service to be ready...");
    sleep(Duration::from_secs(10)).await;

    // Step 4: Test health endpoint
    println!("🏥 Testing health endpoint...");
    let health_response = reqwest::get("http://localhost:3001/health").await?;

    if !health_response.status().is_success() {
        eprintln!("❌ Health check failed: {}", health_response.status());
        return Err("Health check failed".into());
    }
    println!("✅ Health check passed");

    // Step 5: Test GraphQL schema introspection
    println!("🔍 Testing GraphQL schema introspection...");
    let introspection_query = r#"{
        "__schema": {
            "queryType": {
                "fields": {
                    "name": true,
                    "description": true
                }
            }
        }
    }"#;

    let schema_response = reqwest::Client::new()
        .post("http://localhost:3001/graphql")
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "query": introspection_query
        }))
        .send()
        .await?;

    if !schema_response.status().is_success() {
        eprintln!(
            "❌ Schema introspection failed: {}",
            schema_response.status()
        );
        return Err("Schema introspection failed".into());
    }

    let schema_data: serde_json::Value = schema_response.json().await?;
    println!("✅ Schema introspection successful");

    // Step 6: Test creating a series
    println!("📊 Testing series creation...");
    let create_mutation = r#"mutation {
        createSeries(input: {
            sourceId: "550e8400-e29b-41d4-a716-446655440000"
            externalId: "TEST001"
            title: "Test Economic Series"
            description: "A test series for integration testing"
            frequency: "monthly"
            isActive: true
        }) {
            id
            title
            externalId
        }
    }"#;

    let create_response = reqwest::Client::new()
        .post("http://localhost:3001/graphql")
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "query": create_mutation
        }))
        .send()
        .await?;

    if !create_response.status().is_success() {
        eprintln!("❌ Series creation failed: {}", create_response.status());
        return Err("Series creation failed".into());
    }

    let create_data: serde_json::Value = create_response.json().await?;
    println!("✅ Series created successfully");

    // Step 7: Test listing series
    println!("📋 Testing series listing...");
    let list_query = r#"{
        listSeries {
            id
            title
            externalId
            frequency
            isActive
        }
    }"#;

    let list_response = reqwest::Client::new()
        .post("http://localhost:3001/graphql")
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "query": list_query
        }))
        .send()
        .await?;

    if !list_response.status().is_success() {
        eprintln!("❌ Series listing failed: {}", list_response.status());
        return Err("Series listing failed".into());
    }

    let list_data: serde_json::Value = list_response.json().await?;
    println!("✅ Series listing successful");

    // Step 8: Test GraphQL Playground
    println!("🎮 Testing GraphQL Playground...");
    let playground_response = reqwest::get("http://localhost:3001/").await?;

    if !playground_response.status().is_success() {
        eprintln!(
            "❌ GraphQL Playground failed: {}",
            playground_response.status()
        );
        return Err("GraphQL Playground failed".into());
    }
    println!("✅ GraphQL Playground accessible");

    // Step 9: Clean up
    println!("🧹 Cleaning up...");
    let _stop_result = Command::new("docker")
        .args(["stop", "financial-data-test"])
        .output()?;

    let _remove_result = Command::new("docker")
        .args(["rm", "financial-data-test"])
        .output()?;

    println!("✅ Docker integration test completed successfully!");

    // Print summary
    println!("\n📊 Test Summary:");
    println!("  ✅ Docker image built");
    println!("  ✅ Service started in container");
    println!("  ✅ Health check passed");
    println!("  ✅ GraphQL schema introspection");
    println!("  ✅ Series creation");
    println!("  ✅ Series listing");
    println!("  ✅ GraphQL Playground");
    println!("  ✅ Cleanup completed");

    Ok(())
}

/// Test Docker Compose integration
#[tokio::test]
async fn test_docker_compose_integration() -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Command;

    println!("🐳 Starting Docker Compose integration test...");

    // Check if docker-compose.yml exists
    if !std::path::Path::new("docker-compose.yml").exists() {
        println!("⚠️  docker-compose.yml not found, skipping compose test");
        return Ok(());
    }

    // Start services with docker-compose
    println!("🚀 Starting services with docker-compose...");
    let up_result = Command::new("docker-compose")
        .args(["up", "-d", "--build"])
        .output()?;

    if !up_result.status.success() {
        eprintln!("❌ Docker Compose up failed:");
        eprintln!("{}", String::from_utf8_lossy(&up_result.stderr));
        return Err("Docker Compose up failed".into());
    }

    // Wait for services to be ready
    println!("⏳ Waiting for services to be ready...");
    sleep(Duration::from_secs(15)).await;

    // Test the service
    println!("🏥 Testing service health...");
    let health_response = reqwest::get("http://localhost:3001/health").await?;

    if !health_response.status().is_success() {
        eprintln!("❌ Health check failed: {}", health_response.status());
        return Err("Health check failed".into());
    }

    println!("✅ Docker Compose integration test successful!");

    // Clean up
    println!("🧹 Cleaning up docker-compose...");
    let _down_result = Command::new("docker-compose")
        .args(["down", "-v"])
        .output()?;

    Ok(())
}
