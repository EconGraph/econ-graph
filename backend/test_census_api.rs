use reqwest;
use serde_json;
use std::time::Duration;
use tokio::time::timeout;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BdsDataPoint {
    pub variable: String,
    pub year: i32,
    pub value: Option<i64>,
    pub geography: String,
}

fn parse_census_response(response: &str) -> Result<Vec<BdsDataPoint>, Box<dyn std::error::Error>> {
    let raw_data: Vec<Vec<String>> = serde_json::from_str(response)?;

    if raw_data.len() < 2 {
        return Err("Invalid Census response: expected at least header and data rows".into());
    }

    let headers = &raw_data[0];
    let data_rows = &raw_data[1..];

    // Find indices of key columns
    let year_idx = headers
        .iter()
        .position(|h| h.to_lowercase() == "year")
        .ok_or("YEAR column not found in response")?;

    // Census API returns geography as the last column with numeric codes
    let geo_idx = headers.len() - 1;

    let mut data_points = Vec::new();

    for row in data_rows {
        if row.len() != headers.len() {
            continue; // Skip malformed rows
        }

        // Parse year
        let year: i32 = row[year_idx].parse().unwrap_or(0);
        if year == 0 {
            continue; // Skip invalid years
        }

        // Parse geography (last column)
        let geography = row[geo_idx].clone();

        // Parse each variable (excluding YEAR and geography columns)
        for (i, header) in headers.iter().enumerate() {
            if header.to_lowercase() == "year" || i == geo_idx {
                continue; // Skip YEAR and geography columns
            }

            let value = if row[i].is_empty() {
                None
            } else {
                row[i].parse().ok()
            };

            data_points.push(BdsDataPoint {
                variable: header.clone(),
                year,
                value,
                geography: geography.clone(),
            });
        }
    }

    Ok(data_points)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔍 Testing Census BDS API integration...");

    let client = reqwest::Client::new();

    // Test 1: Basic API call
    println!("\n📊 Test 1: Basic API call");
    let url = "https://api.census.gov/data/timeseries/bds?get=ESTAB,YEAR&for=us&YEAR=2020";
    println!("📡 URL: {}", url);

    let result = timeout(
        Duration::from_secs(30),
        client.get(url).send(),
    ).await;

    match result {
        Ok(Ok(resp)) => {
            println!("✅ Got response with status: {}", resp.status());

            if resp.status().is_success() {
                let text = resp.text().await?;
                println!("📄 Response length: {} characters", text.len());
                println!("📄 Raw response: {}", text);

                // Parse the response
                match parse_census_response(&text) {
                    Ok(data_points) => {
                        println!("✅ Successfully parsed {} data points", data_points.len());
                        for point in &data_points {
                            println!("📈 Data point: variable={}, year={}, value={:?}, geography={}",
                                point.variable, point.year, point.value, point.geography);
                        }
                    }
                    Err(e) => {
                        println!("❌ Failed to parse response: {}", e);
                    }
                }
            } else {
                println!("❌ API returned error status: {}", resp.status());
            }
        }
        Ok(Err(e)) => {
            println!("❌ HTTP request failed: {}", e);
        }
        Err(_) => {
            println!("⏰ HTTP request timed out");
        }
    }

    // Test 2: Multiple variables
    println!("\n📊 Test 2: Multiple variables");
    let url2 = "https://api.census.gov/data/timeseries/bds?get=ESTAB,FIRM,YEAR&for=us&YEAR=2020,2021";
    println!("📡 URL: {}", url2);

    let result2 = timeout(
        Duration::from_secs(30),
        client.get(url2).send(),
    ).await;

    match result2 {
        Ok(Ok(resp)) => {
            println!("✅ Got response with status: {}", resp.status());

            if resp.status().is_success() {
                let text = resp.text().await?;
                println!("📄 Response length: {} characters", text.len());

                // Parse the response
                match parse_census_response(&text) {
                    Ok(data_points) => {
                        println!("✅ Successfully parsed {} data points", data_points.len());
                        for point in &data_points {
                            println!("📈 Data point: variable={}, year={}, value={:?}, geography={}",
                                point.variable, point.year, point.value, point.geography);
                        }
                    }
                    Err(e) => {
                        println!("❌ Failed to parse response: {}", e);
                    }
                }
            } else {
                println!("❌ API returned error status: {}", resp.status());
            }
        }
        Ok(Err(e)) => {
            println!("❌ HTTP request failed: {}", e);
        }
        Err(_) => {
            println!("⏰ HTTP request timed out");
        }
    }

    println!("\n🎉 Census API integration test completed!");
    Ok(())
}
