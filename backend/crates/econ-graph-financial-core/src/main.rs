//! Main entry point for econ-graph-financial-core
//!
//! TODO: Add web server and GraphQL support once dependencies are resolved

use anyhow::Result;
use tracing::info;

mod catalog;
mod crawler;
mod database;
mod models;
mod monitoring;
mod storage;

use crate::database::Database;
use crate::monitoring::{HealthChecker, MetricsCollector};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    info!("Starting econ-graph-financial-core");

    // Create database with in-memory storage for now
    let _database = Database::new_in_memory().await?;

    // Initialize monitoring
    let _health_checker = HealthChecker::new();
    let _metrics_collector = MetricsCollector::new();

    info!("Financial data service started successfully");
    info!("Database: In-memory storage");
    info!("Monitoring: Health checks and metrics collection enabled");

    // Keep the service running
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        info!("Service is running...");
    }
}
