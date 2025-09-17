// REQUIREMENT: Comprehensive crawler that can discover and catalog all available series
// PURPOSE: Automatically discover, catalog, and crawl economic data from all supported sources
// This replaces the hardcoded series list with dynamic discovery

use crate::services::crawler::legacy_crawler_service::CrawlerService;
use crate::services::series_discovery::SeriesDiscoveryService;
use econ_graph_core::database::DatabasePool;
use econ_graph_core::error::AppResult;
use econ_graph_core::models::{CrawlQueueItem, NewCrawlQueueItem, QueuePriority};

/// Comprehensive crawler that can discover and catalog all available series
pub struct ComprehensiveCrawler {
    discovery_service: SeriesDiscoveryService,
    crawler_service: CrawlerService,
}

impl ComprehensiveCrawler {
    pub fn new(
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
            crawler_service: CrawlerService::new(fred_api_key, bls_api_key),
        }
    }

    /// Discover and catalog all available series from all sources
    pub async fn discover_all_series(&self, pool: &DatabasePool) -> AppResult<DiscoveryResults> {
        let mut results = DiscoveryResults {
            fred_series: Vec::new(),
            bls_series: Vec::new(),
            census_series: Vec::new(),
            bea_series: Vec::new(),
            world_bank_series: Vec::new(),
            imf_series: Vec::new(),
            total_discovered: 0,
        };

        println!("Starting comprehensive series discovery...");

        // Discover FRED series
        match self.discovery_service.discover_fred_series(pool).await {
            Ok(series) => {
                results.fred_series = series;
                println!("✅ Discovered {} FRED series", results.fred_series.len());
            }
            Err(e) => {
                println!("❌ FRED discovery failed: {}", e);
            }
        }

        // Discover BLS series (limited)
        match self.discovery_service.discover_bls_series(pool).await {
            Ok(series) => {
                results.bls_series = series;
                println!("✅ Discovered {} BLS series", results.bls_series.len());
            }
            Err(e) => {
                println!("❌ BLS discovery failed: {}", e);
            }
        }

        // Discover Census series
        match self.discovery_service.discover_census_series(pool).await {
            Ok(series) => {
                results.census_series = series;
                println!(
                    "✅ Discovered {} Census series",
                    results.census_series.len()
                );
            }
            Err(e) => {
                println!("❌ Census discovery failed: {}", e);
                results.census_series = Vec::new();
            }
        }

        // Discover BEA series
        match self.discovery_service.discover_bea_series(pool).await {
            Ok(series) => {
                results.bea_series = series;
                println!("✅ Discovered {} BEA series", results.bea_series.len());
            }
            Err(e) => {
                println!("❌ BEA discovery failed: {}", e);
                results.bea_series = Vec::new();
            }
        }

        // Discover World Bank series
        match self
            .discovery_service
            .discover_world_bank_series(pool)
            .await
        {
            Ok(series) => {
                results.world_bank_series = series;
                println!(
                    "✅ Discovered {} World Bank series",
                    results.world_bank_series.len()
                );
            }
            Err(e) => {
                println!("❌ World Bank discovery failed: {}", e);
                results.world_bank_series = Vec::new();
            }
        }

        // Discover IMF series
        match self.discovery_service.discover_imf_series(pool).await {
            Ok(series) => {
                results.imf_series = series;
                println!("✅ Discovered {} IMF series", results.imf_series.len());
            }
            Err(e) => {
                println!("❌ IMF discovery failed: {}", e);
                results.imf_series = Vec::new();
            }
        }

        results.total_discovered = results.fred_series.len()
            + results.bls_series.len()
            + results.census_series.len()
            + results.bea_series.len()
            + results.world_bank_series.len()
            + results.imf_series.len();

        println!(
            "🎉 Discovery complete: {} total series found",
            results.total_discovered
        );
        Ok(results)
    }

    /// Queue all discovered series for crawling
    pub async fn queue_all_series_for_crawling(&self, pool: &DatabasePool) -> AppResult<()> {
        println!("Queueing all discovered series for crawling...");

        // Get all series from database that are active
        let all_series = self.get_all_active_series(pool).await?;

        let mut queued_count = 0;
        for series in all_series {
            // Determine source from series ID or source_id
            let source = self
                .determine_source(&series.external_id, series.source_id)
                .await?;

            // Add to crawl queue
            let queue_item = NewCrawlQueueItem {
                series_id: series.external_id,
                source,
                priority: QueuePriority::Normal.into(),
                scheduled_for: Some(chrono::Utc::now()),
                max_retries: 3,
            };

            CrawlQueueItem::create(pool, &queue_item).await?;
            queued_count += 1;
        }

        println!("✅ Queued {} series for crawling", queued_count);
        Ok(())
    }

    /// Run comprehensive discovery and crawling process
    pub async fn run_full_discovery_and_crawl(&self, pool: &DatabasePool) -> AppResult<()> {
        println!("🚀 Starting full discovery and crawl process...");

        // Step 1: Discover all available series
        let _discovery_results = self.discover_all_series(pool).await?;

        // Step 2: Queue all series for crawling
        self.queue_all_series_for_crawling(pool).await?;

        // Step 3: Start crawling process
        println!("🔄 Starting crawling process...");
        self.crawler_service
            .process_queue(pool, "comprehensive_crawler")
            .await?;

        println!("✅ Full discovery and crawl process completed");
        Ok(())
    }

    /// Get all active series from database
    async fn get_all_active_series(
        &self,
        pool: &DatabasePool,
    ) -> AppResult<Vec<econ_graph_core::models::EconomicSeries>> {
        use diesel::prelude::*;
        use diesel_async::RunQueryDsl;
        use econ_graph_core::schema::economic_series::dsl::*;

        let mut conn = pool.get().await?;
        let series_list = economic_series
            .filter(is_active.eq(true))
            .select(econ_graph_core::models::EconomicSeries::as_select())
            .load::<econ_graph_core::models::EconomicSeries>(&mut conn)
            .await?;

        Ok(series_list)
    }

    /// Determine source from series ID pattern or source_id
    async fn determine_source(
        &self,
        external_id: &str,
        _source_id: uuid::Uuid,
    ) -> AppResult<String> {
        // Try to determine from series ID pattern first
        if external_id.len() >= 10 && external_id.chars().all(|c| c.is_alphanumeric()) {
            // FRED series IDs are typically 10+ alphanumeric characters
            return Ok("FRED".to_string());
        }

        // Fall back to database lookup - this would need a proper database connection
        // For now, return a placeholder based on series ID patterns
        if external_id.starts_with("LNS")
            || external_id.starts_with("CES")
            || external_id.starts_with("JTS")
        {
            Ok("BLS".to_string())
        } else if external_id.starts_with("GDP") || external_id.starts_with("UNRATE") {
            Ok("FRED".to_string())
        } else {
            Ok("UNKNOWN".to_string())
        }
    }

    /// Search for series by keyword across all sources
    pub async fn search_series(&self, query: &str) -> AppResult<SearchResults> {
        let mut results = SearchResults {
            fred_series: Vec::new(),
            bls_series: Vec::new(),
            total_found: 0,
        };

        // Search FRED
        match self.discovery_service.search_fred_series(query).await {
            Ok(series) => {
                results.fred_series = series;
            }
            Err(e) => {
                println!("FRED search failed: {}", e);
            }
        }

        // BLS search would require different approach
        results.bls_series = Vec::new();

        results.total_found = results.fred_series.len() + results.bls_series.len();
        Ok(results)
    }
}

/// Results from series discovery process
#[derive(Debug, Clone)]
pub struct DiscoveryResults {
    pub fred_series: Vec<String>,
    pub bls_series: Vec<String>,
    pub census_series: Vec<String>,
    pub bea_series: Vec<String>,
    pub world_bank_series: Vec<String>,
    pub imf_series: Vec<String>,
    pub total_discovered: usize,
}

/// Results from series search
#[derive(Debug)]
pub struct SearchResults {
    pub fred_series: Vec<crate::services::series_discovery::fred::FredSeriesInfo>,
    pub bls_series: Vec<String>,
    pub total_found: usize,
}
