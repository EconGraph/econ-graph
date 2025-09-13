//! Comprehensive crawler scheduler that integrates series discovery, crawl attempts tracking,
//! and data source visibility controls for intelligent, predictive crawling

use std::collections::HashMap;
use tokio::time::{sleep, Duration};
use uuid::Uuid;

use diesel::prelude::*;
use diesel::ExpressionMethods;
use diesel_async::RunQueryDsl;
// Removed futures_util import as we're using .await? instead of .map_err()

use crate::database::DatabasePool;
use crate::error::{AppError, AppResult};
use crate::services::{
    enhanced_crawler_service::{CrawlableSeries, EnhancedCrawlerService},
    series_discovery::SeriesDiscoveryService,
};

/// Comprehensive crawler scheduler with intelligent crawling strategies
pub struct ComprehensiveCrawlerScheduler {
    discovery_service: SeriesDiscoveryService,
    crawler_service: EnhancedCrawlerService,
    pool: DatabasePool,
}

impl ComprehensiveCrawlerScheduler {
    pub fn new(
        pool: DatabasePool,
        fred_api_key: Option<String>,
        bls_api_key: Option<String>,
        census_api_key: Option<String>,
        bea_api_key: Option<String>,
    ) -> Self {
        Self {
            discovery_service: SeriesDiscoveryService::new(
                fred_api_key.clone(),
                bls_api_key.clone(),
                census_api_key.clone(),
                bea_api_key.clone(),
            ),
            crawler_service: EnhancedCrawlerService::new(fred_api_key, bls_api_key),
            pool,
        }
    }

    /// Run the comprehensive crawling process
    pub async fn run_comprehensive_crawling(&self) -> AppResult<CrawlingReport> {
        println!("🚀 Starting comprehensive crawling process...");

        let mut report = CrawlingReport {
            discovery_results: None,
            crawl_results: Vec::new(),
            total_series_crawled: 0,
            total_new_data_points: 0,
            errors: Vec::new(),
            start_time: chrono::Utc::now(),
            end_time: None,
        };

        // Step 1: Discover new series
        println!("📡 Step 1: Discovering new series...");
        match self.discovery_service.discover_all_series(&self.pool).await {
            Ok(discovered_series) => {
                // Create a DiscoveryResults struct from the discovered series
                let discovery_results = crate::services::comprehensive_crawler::DiscoveryResults {
                    fred_series: Vec::new(), // TODO: Separate by source
                    bls_series: Vec::new(),
                    census_series: Vec::new(),
                    bea_series: Vec::new(),
                    world_bank_series: Vec::new(),
                    imf_series: Vec::new(),
                    total_discovered: discovered_series.len(),
                };
                report.discovery_results = Some(discovery_results);
                println!(
                    "✅ Discovery complete: {} total series found",
                    discovered_series.len()
                );
            }
            Err(e) => {
                let error_msg = format!("Series discovery failed: {}", e);
                println!("❌ {}", error_msg);
                report.errors.push(error_msg);
            }
        }

        // Step 2: Get crawlable series based on intelligent scheduling
        println!("🎯 Step 2: Identifying series to crawl...");
        let crawlable_series = self.get_intelligent_crawl_schedule().await?;
        println!(
            "📋 Found {} series ready for crawling",
            crawlable_series.len()
        );

        // Step 3: Crawl series with tracking
        println!("🔄 Step 3: Crawling series with comprehensive tracking...");
        for series in crawlable_series {
            match self
                .crawler_service
                .crawl_series_with_tracking(
                    &self.pool,
                    &series.series_id,
                    &series.external_id,
                    &series.source_name,
                )
                .await
            {
                Ok(crawl_result) => {
                    let series_title = series.title.clone();
                    report.crawl_results.push(CrawlResult {
                        series_id: series.series_id,
                        series_title: series.title,
                        source_name: series.source_name,
                        success: true,
                        new_data_points: crawl_result.new_data_points,
                        data_found: crawl_result.data_found,
                        error_message: None,
                    });
                    report.total_series_crawled += 1;
                    report.total_new_data_points += crawl_result.new_data_points as usize;

                    println!(
                        "✅ Crawled {}: {} new data points",
                        series_title, crawl_result.new_data_points
                    );
                }
                Err(e) => {
                    let error_msg = format!("Failed to crawl {}: {}", series.title, e);
                    println!("❌ {}", error_msg);
                    report.errors.push(error_msg.clone());

                    report.crawl_results.push(CrawlResult {
                        series_id: series.series_id,
                        series_title: series.title,
                        source_name: series.source_name,
                        success: false,
                        new_data_points: 0,
                        data_found: false,
                        error_message: Some(error_msg),
                    });
                }
            }

            // Small delay to avoid overwhelming APIs
            sleep(Duration::from_millis(100)).await;
        }

        report.end_time = Some(chrono::Utc::now());

        // Step 4: Update data source crawl status
        self.update_data_source_crawl_status().await?;

        // Step 5: Generate crawling insights
        self.generate_crawling_insights(&report).await?;

        println!("🎉 Comprehensive crawling complete!");
        println!(
            "📊 Summary: {} series crawled, {} new data points, {} errors",
            report.total_series_crawled,
            report.total_new_data_points,
            report.errors.len()
        );

        Ok(report)
    }

    /// Get intelligent crawl schedule based on historical data and data source settings
    async fn get_intelligent_crawl_schedule(&self) -> AppResult<Vec<CrawlableSeries>> {
        let mut crawlable_series = self
            .crawler_service
            .get_crawlable_series(&self.pool)
            .await?;

        // Sort by priority based on intelligent factors
        // For now, use a simple priority calculation without async
        crawlable_series.sort_by(|a, b| {
            // Priority factors (higher priority first):
            // 1. Never crawled before (last_crawled_at is NULL)
            // 2. High success rate series
            // 3. Recently failed series (for retry)
            // 4. Data freshness requirements

            let a_priority = if a.last_crawled_at.is_none() { 100 } else { 50 };
            let b_priority = if b.last_crawled_at.is_none() { 100 } else { 50 };

            b_priority.cmp(&a_priority)
        });

        // Limit to reasonable batch size to avoid overwhelming APIs
        crawlable_series.truncate(100);

        Ok(crawlable_series)
    }

    /// Calculate priority score for a series based on intelligent factors
    async fn calculate_series_priority(&self, series: &CrawlableSeries) -> AppResult<i32> {
        let mut priority = 0;

        // Factor 1: Never crawled before (highest priority)
        if series.last_crawled_at.is_none() {
            priority += 1000;
        }

        // Factor 2: Get crawl statistics for intelligent decisions
        if let Ok(stats) = self
            .crawler_service
            .get_series_crawl_statistics(&self.pool, &series.series_id)
            .await
        {
            // High success rate series get higher priority
            if stats.success_rate > 0.8 {
                priority += 500;
            } else if stats.success_rate > 0.5 {
                priority += 200;
            }

            // Recently failed series get retry priority
            if let Some(last_attempt) = &stats.last_attempt {
                if !last_attempt.success && last_attempt.retry_count.unwrap_or(0) < 3 {
                    priority += 300;
                }
            }

            // Data freshness factor
            if let Some(avg_freshness) = stats.avg_freshness_hours {
                if avg_freshness < 24.0 {
                    priority += 400; // Fresh data, crawl more frequently
                } else if avg_freshness > 168.0 {
                    priority += 100; // Stale data, lower priority
                }
            }
        }

        // Factor 3: Data source frequency requirements
        priority += series.crawl_frequency_hours;

        Ok(priority)
    }

    /// Update data source crawl status after crawling
    async fn update_data_source_crawl_status(&self) -> AppResult<()> {
        let mut conn = self
            .pool
            .get()
            .await
            .map_err(|e| AppError::DatabaseError(e.to_string()))?;

        // Update last_crawl_at for all enabled data sources
        diesel::update(
            crate::schema::data_sources::table
                .filter(crate::schema::data_sources::is_enabled.eq(true)),
        )
        .set((
            crate::schema::data_sources::last_crawl_at.eq(Some(chrono::Utc::now())),
            crate::schema::data_sources::crawl_status.eq(Some("completed".to_string())),
            crate::schema::data_sources::crawl_error_message.eq(None::<String>),
            crate::schema::data_sources::updated_at.eq(chrono::Utc::now()),
        ))
        .execute(&mut conn)
        .await?;

        Ok(())
    }

    /// Generate insights and recommendations based on crawling results
    async fn generate_crawling_insights(&self, report: &CrawlingReport) -> AppResult<()> {
        println!("📈 Generating crawling insights...");

        // Analyze success rates by data source
        let mut source_stats: HashMap<String, (usize, usize)> = HashMap::new();

        for result in &report.crawl_results {
            let entry = source_stats
                .entry(result.source_name.clone())
                .or_insert((0, 0));
            entry.0 += 1; // total attempts
            if result.success {
                entry.1 += 1; // successful attempts
            }
        }

        println!("📊 Success rates by data source:");
        for (source, (total, successful)) in source_stats {
            let success_rate = if total > 0 {
                (successful as f64 / total as f64) * 100.0
            } else {
                0.0
            };
            println!(
                "  {}: {:.1}% ({}/{} successful)",
                source, success_rate, successful, total
            );
        }

        // Analyze data freshness patterns
        let total_data_points = report.total_new_data_points;
        let avg_data_per_series = if report.total_series_crawled > 0 {
            total_data_points as f64 / report.total_series_crawled as f64
        } else {
            0.0
        };

        println!("📊 Data freshness insights:");
        println!(
            "  Average new data points per series: {:.1}",
            avg_data_per_series
        );
        println!("  Total new data points: {}", total_data_points);

        // Generate recommendations
        if report.errors.len() > report.total_series_crawled / 4 {
            println!("⚠️  High error rate detected. Consider:");
            println!("   - Checking API keys and rate limits");
            println!("   - Reviewing network connectivity");
            println!("   - Adjusting crawl frequency for problematic sources");
        }

        if avg_data_per_series < 1.0 {
            println!("💡 Low data yield detected. Consider:");
            println!("   - Increasing crawl frequency for high-value series");
            println!("   - Reviewing series selection criteria");
            println!("   - Checking for data source changes");
        }

        Ok(())
    }

    /// Run continuous crawling with intelligent scheduling
    pub async fn run_continuous_crawling(&self, interval_minutes: u64) -> AppResult<()> {
        println!(
            "🔄 Starting continuous crawling (interval: {} minutes)",
            interval_minutes
        );

        loop {
            match self.run_comprehensive_crawling().await {
                Ok(report) => {
                    println!("✅ Crawling cycle completed successfully");
                    if !report.errors.is_empty() {
                        println!(
                            "⚠️  {} errors occurred during crawling",
                            report.errors.len()
                        );
                    }
                }
                Err(e) => {
                    println!("❌ Crawling cycle failed: {}", e);
                }
            }

            println!(
                "⏰ Waiting {} minutes until next crawling cycle...",
                interval_minutes
            );
            sleep(Duration::from_secs(interval_minutes * 60)).await;
        }
    }
}

/// Comprehensive crawling report
#[derive(Debug, Clone)]
pub struct CrawlingReport {
    pub discovery_results: Option<crate::services::comprehensive_crawler::DiscoveryResults>,
    pub crawl_results: Vec<CrawlResult>,
    pub total_series_crawled: usize,
    pub total_new_data_points: usize,
    pub errors: Vec<String>,
    pub start_time: chrono::DateTime<chrono::Utc>,
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
}

/// Individual crawl result
#[derive(Debug, Clone)]
pub struct CrawlResult {
    pub series_id: Uuid,
    pub series_title: String,
    pub source_name: String,
    pub success: bool,
    pub new_data_points: i32,
    pub data_found: bool,
    pub error_message: Option<String>,
}
