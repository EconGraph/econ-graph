//! GraphQL client examples for the Financial Data Service
//!
//! This example demonstrates how to interact with the service using GraphQL queries and mutations.
//! It shows both direct HTTP requests and using a GraphQL client library.

use anyhow::Result;
use reqwest::Client;
use serde_json::json;
use std::time::Duration;
use uuid::Uuid;

const SERVICE_URL: &str = "http://localhost:3001/graphql";

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    println!("🚀 Financial Data Service - GraphQL Client Examples");
    println!("==================================================\n");

    // Wait for service to be ready
    println!("⏳ Waiting for service to be ready...");
    wait_for_service().await?;

    // Example 1: Health check
    println!("🏥 Example 1: Health Check");
    example_health_check().await?;

    // Example 2: Create a series
    println!("\n📊 Example 2: Create Economic Series");
    let series_id = example_create_series().await?;

    // Example 3: Add data points
    println!("\n📈 Example 3: Add Data Points");
    example_add_data_points(series_id).await?;

    // Example 4: Query data
    println!("\n🔍 Example 4: Query Data");
    example_query_data(series_id).await?;

    // Example 5: List all series
    println!("\n📋 Example 5: List All Series");
    example_list_series().await?;

    // Example 6: Error handling
    println!("\n⚠️ Example 6: Error Handling");
    example_error_handling().await?;

    println!("\n✅ All GraphQL examples completed successfully!");
    Ok(())
}

async fn wait_for_service() -> Result<()> {
    let client = Client::new();
    let max_attempts = 30;
    let delay = Duration::from_secs(1);

    for attempt in 1..=max_attempts {
        match client.get("http://localhost:3001/health").send().await {
            Ok(response) if response.status().is_success() => {
                println!("✅ Service is ready!");
                return Ok(());
            }
            Ok(response) => {
                println!(
                    "⏳ Attempt {}/{}: Service responded with status {}",
                    attempt,
                    max_attempts,
                    response.status()
                );
            }
            Err(e) => {
                println!(
                    "⏳ Attempt {}/{}: Service not ready yet ({})",
                    attempt, max_attempts, e
                );
            }
        }

        if attempt < max_attempts {
            tokio::time::sleep(delay).await;
        }
    }

    Err(anyhow::anyhow!(
        "Service did not become ready within {} seconds",
        max_attempts
    ))
}

async fn example_health_check() -> Result<()> {
    let client = Client::new();

    let query = json!({
        "query": r#"
            query {
                health
            }
        "#
    });

    let response = client.post(SERVICE_URL).json(&query).send().await?;

    let result: serde_json::Value = response.json().await?;
    println!("✅ Health check response: {}", result["data"]["health"]);

    Ok(())
}

async fn example_create_series() -> Result<Uuid> {
    let client = Client::new();
    let source_id = Uuid::new_v4();
    let series_id = Uuid::new_v4();

    let mutation = json!({
        "query": r#"
            mutation CreateSeries($input: CreateEconomicSeriesInput!) {
                createSeries(input: $input) {
                    id
                    title
                    frequency
                    isActive
                }
            }
        "#,
        "variables": {
            "input": {
                "sourceId": source_id.to_string(),
                "externalId": "GDP_US",
                "title": "US Gross Domestic Product",
                "description": "Quarterly GDP data for the United States",
                "units": "Billions of USD",
                "frequency": "QUARTERLY",
                "seasonalAdjustment": "SAAR",
                "startDate": "1947-01-01",
                "endDate": "2023-12-31",
                "isActive": true
            }
        }
    });

    let response = client.post(SERVICE_URL).json(&mutation).send().await?;

    let result: serde_json::Value = response.json().await?;

    if let Some(errors) = result.get("errors") {
        println!("❌ GraphQL errors: {}", errors);
        return Err(anyhow::anyhow!("GraphQL errors occurred"));
    }

    let series = &result["data"]["createSeries"];
    let created_id = Uuid::parse_str(series["id"].as_str().unwrap())?;

    println!("✅ Created series: {} ({})", series["title"], series["id"]);
    println!("   Frequency: {}", series["frequency"]);
    println!("   Active: {}", series["isActive"]);

    Ok(created_id)
}

async fn example_add_data_points(series_id: Uuid) -> Result<()> {
    let client = Client::new();

    let mutation = json!({
        "query": r#"
            mutation CreateDataPoints($inputs: [CreateDataPointInput!]!) {
                createDataPoints(inputs: $inputs) {
                    id
                    date
                    value
                    isOriginalRelease
                }
            }
        "#,
        "variables": {
            "inputs": [
                {
                    "seriesId": series_id.to_string(),
                    "date": "2023-01-01",
                    "value": "25000.5",
                    "revisionDate": "2023-01-15",
                    "isOriginalRelease": true
                },
                {
                    "seriesId": series_id.to_string(),
                    "date": "2023-04-01",
                    "value": "25100.2",
                    "revisionDate": "2023-04-15",
                    "isOriginalRelease": true
                },
                {
                    "seriesId": series_id.to_string(),
                    "date": "2023-07-01",
                    "value": "25200.8",
                    "revisionDate": "2023-07-15",
                    "isOriginalRelease": true
                },
                {
                    "seriesId": series_id.to_string(),
                    "date": "2023-10-01",
                    "value": "25300.1",
                    "revisionDate": "2023-10-15",
                    "isOriginalRelease": true
                }
            ]
        }
    });

    let response = client.post(SERVICE_URL).json(&mutation).send().await?;

    let result: serde_json::Value = response.json().await?;

    if let Some(errors) = result.get("errors") {
        println!("❌ GraphQL errors: {}", errors);
        return Err(anyhow::anyhow!("GraphQL errors occurred"));
    }

    let data_points = result["data"]["createDataPoints"].as_array().unwrap();
    println!("✅ Created {} data points", data_points.len());

    for (i, point) in data_points.iter().enumerate() {
        println!("   Point {}: {} = {}", i + 1, point["date"], point["value"]);
    }

    Ok(())
}

async fn example_query_data(series_id: Uuid) -> Result<()> {
    let client = Client::new();

    // Query the series metadata
    let series_query = json!({
        "query": r#"
            query GetSeries($id: ID!) {
                series(id: $id) {
                    id
                    title
                    description
                    units
                    frequency
                    seasonalAdjustment
                    startDate
                    endDate
                    isActive
                }
            }
        "#,
        "variables": {
            "id": series_id.to_string()
        }
    });

    let response = client.post(SERVICE_URL).json(&series_query).send().await?;

    let result: serde_json::Value = response.json().await?;

    if let Some(errors) = result.get("errors") {
        println!("❌ GraphQL errors: {}", errors);
        return Err(anyhow::anyhow!("GraphQL errors occurred"));
    }

    let series = &result["data"]["series"];
    println!(
        "✅ Retrieved series: {} ({})",
        series["title"], series["id"]
    );
    println!("   Description: {}", series["description"]);
    println!("   Units: {}", series["units"]);
    println!("   Frequency: {}", series["frequency"]);
    println!("   Seasonal Adjustment: {}", series["seasonalAdjustment"]);
    println!(
        "   Date Range: {} to {}",
        series["startDate"], series["endDate"]
    );
    println!("   Active: {}", series["isActive"]);

    // Query data points
    let data_points_query = json!({
        "query": r#"
            query GetDataPoints($seriesId: ID!, $startDate: Date, $endDate: Date) {
                dataPoints(seriesId: $seriesId, startDate: $startDate, endDate: $endDate) {
                    id
                    date
                    value
                    revisionDate
                    isOriginalRelease
                }
            }
        "#,
        "variables": {
            "seriesId": series_id.to_string(),
            "startDate": "2023-01-01",
            "endDate": "2023-12-31"
        }
    });

    let response = client
        .post(SERVICE_URL)
        .json(&data_points_query)
        .send()
        .await?;

    let result: serde_json::Value = response.json().await?;

    if let Some(errors) = result.get("errors") {
        println!("❌ GraphQL errors: {}", errors);
        return Err(anyhow::anyhow!("GraphQL errors occurred"));
    }

    let data_points = result["data"]["dataPoints"].as_array().unwrap();
    println!("✅ Retrieved {} data points", data_points.len());

    for point in data_points {
        println!(
            "   {}: {} (revised: {}, original: {})",
            point["date"], point["value"], point["revisionDate"], point["isOriginalRelease"]
        );
    }

    Ok(())
}

async fn example_list_series() -> Result<()> {
    let client = Client::new();

    let query = json!({
        "query": r#"
            query {
                listSeries {
                    id
                    title
                    frequency
                    isActive
                }
            }
        "#
    });

    let response = client.post(SERVICE_URL).json(&query).send().await?;

    let result: serde_json::Value = response.json().await?;

    if let Some(errors) = result.get("errors") {
        println!("❌ GraphQL errors: {}", errors);
        return Err(anyhow::anyhow!("GraphQL errors occurred"));
    }

    let series_list = result["data"]["listSeries"].as_array().unwrap();
    println!("✅ Found {} series", series_list.len());

    for (i, series) in series_list.iter().enumerate() {
        println!(
            "   {}: {} ({}) - {}",
            i + 1,
            series["title"],
            series["frequency"],
            if series["isActive"].as_bool().unwrap() {
                "Active"
            } else {
                "Inactive"
            }
        );
    }

    Ok(())
}

async fn example_error_handling() -> Result<()> {
    let client = Client::new();

    // Try to query a non-existent series
    let query = json!({
        "query": r#"
            query GetSeries($id: ID!) {
                series(id: $id) {
                    id
                    title
                }
            }
        "#,
        "variables": {
            "id": "00000000-0000-0000-0000-000000000000"
        }
    });

    let response = client.post(SERVICE_URL).json(&query).send().await?;

    let result: serde_json::Value = response.json().await?;

    if let Some(errors) = result.get("errors") {
        println!("❌ GraphQL errors (expected): {}", errors);
    } else {
        let series = &result["data"]["series"];
        if series.is_null() {
            println!("✅ Correctly returned null for non-existent series");
        } else {
            println!("❌ Unexpected series found: {}", series);
        }
    }

    // Try to create a series with invalid data
    let invalid_mutation = json!({
        "query": r#"
            mutation CreateSeries($input: CreateEconomicSeriesInput!) {
                createSeries(input: $input) {
                    id
                    title
                }
            }
        "#,
        "variables": {
            "input": {
                "sourceId": "invalid-uuid",
                "externalId": "",
                "title": "",
                "frequency": "INVALID"
            }
        }
    });

    let response = client
        .post(SERVICE_URL)
        .json(&invalid_mutation)
        .send()
        .await?;

    let result: serde_json::Value = response.json().await?;

    if let Some(errors) = result.get("errors") {
        println!("✅ Correctly caught validation errors: {}", errors);
    } else {
        println!("❌ Should have failed validation");
    }

    Ok(())
}
