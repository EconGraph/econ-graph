// REQUIREMENT: Enhanced crawler scheduler for comprehensive series crawling
// PURPOSE: Orchestrate systematic data collection across all economic indicators
// This provides intelligent scheduling, priority management, and error handling

use crate::services::comprehensive_series_catalog::{
    ComprehensiveSeriesCatalog, DataFrequency, DataSource, EconomicCategory, SeriesDefinition,
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use tokio::time::{sleep, Duration as TokioDuration};
use uuid::Uuid;

/// Represents the current status of a crawl job
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CrawlStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Retrying,
    Paused,
}

/// Represents the result of a crawl operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlResult {
    pub series_id: String,
    pub status: CrawlStatus,
    pub records_fetched: usize,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub retry_count: u32,
    pub next_retry: Option<DateTime<Utc>>,
}

/// Configuration for crawl job scheduling
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlJobConfig {
    pub series_id: String,
    pub priority: u8,
    pub frequency: DataFrequency,
    pub source: DataSource,
    pub category: EconomicCategory,
    pub is_active: bool,
    pub retry_limit: u32,
    pub retry_delay_minutes: u32,
    pub last_successful_crawl: Option<DateTime<Utc>>,
    pub next_scheduled_crawl: DateTime<Utc>,
}

/// Comprehensive statistics for crawler operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrawlerStats {
    pub total_series: usize,
    pub active_series: usize,
    pub completed_today: usize,
    pub failed_today: usize,
    pub pending_jobs: usize,
    pub running_jobs: usize,
    pub average_crawl_time_seconds: f64,
    pub success_rate_24h: f64,
    pub next_high_priority_job: Option<DateTime<Utc>>,
    pub estimated_completion_time: Option<DateTime<Utc>>,
}

/// Enhanced crawler scheduler with comprehensive series support
pub struct EnhancedCrawlerScheduler {
    catalog: ComprehensiveSeriesCatalog,
    job_queue: VecDeque<CrawlJobConfig>,
    running_jobs: HashMap<String, CrawlJobConfig>,
    completed_jobs: HashMap<String, CrawlResult>,
    failed_jobs: HashMap<String, CrawlResult>,
    max_concurrent_jobs: usize,
    rate_limit_per_source: HashMap<DataSource, u32>, // requests per minute
    last_request_time: HashMap<DataSource, DateTime<Utc>>,
}

impl EnhancedCrawlerScheduler {
    /// Creates a new enhanced crawler scheduler
    pub fn new() -> Self {
        let catalog = ComprehensiveSeriesCatalog::new();
        let mut rate_limits = HashMap::new();

        // Set conservative rate limits for different data sources
        rate_limits.insert(DataSource::FRED, 120); // FRED allows 120 requests/minute
        rate_limits.insert(DataSource::BLS, 25); // BLS is more restrictive
        rate_limits.insert(DataSource::BEA, 30); // BEA moderate limits
        rate_limits.insert(DataSource::Census, 40); // Census moderate limits
        rate_limits.insert(DataSource::Treasury, 60); // Treasury reasonable limits

        Self {
            catalog,
            job_queue: VecDeque::new(),
            running_jobs: HashMap::new(),
            completed_jobs: HashMap::new(),
            failed_jobs: HashMap::new(),
            max_concurrent_jobs: 5, // Conservative concurrent limit
            rate_limit_per_source: rate_limits,
            last_request_time: HashMap::new(),
        }
    }

    /// Initialize scheduler with all series from catalog
    pub fn initialize_from_catalog(&mut self) {
        let now = Utc::now();

        for series in &self.catalog.series {
            if !series.is_active {
                continue;
            }

            let next_crawl = self.calculate_next_crawl_time(&series.frequency, None);

            let job_config = CrawlJobConfig {
                series_id: series.id.clone(),
                priority: series.priority,
                frequency: series.frequency.clone(),
                source: series.source.clone(),
                category: series.category.clone(),
                is_active: series.is_active,
                retry_limit: 3,
                retry_delay_minutes: self.calculate_retry_delay(series.priority),
                last_successful_crawl: None,
                next_scheduled_crawl: next_crawl,
            };

            self.job_queue.push_back(job_config);
        }

        // Sort job queue by priority and next scheduled time
        self.sort_job_queue();
    }

    /// Calculate when the next crawl should occur based on frequency
    fn calculate_next_crawl_time(
        &self,
        frequency: &DataFrequency,
        last_crawl: Option<DateTime<Utc>>,
    ) -> DateTime<Utc> {
        let base_time = last_crawl.unwrap_or_else(Utc::now);

        match frequency {
            DataFrequency::Daily => base_time + Duration::days(1),
            DataFrequency::Weekly => base_time + Duration::weeks(1),
            DataFrequency::Monthly => base_time + Duration::days(30), // Approximate
            DataFrequency::Quarterly => base_time + Duration::days(90), // Approximate
            DataFrequency::Annual => base_time + Duration::days(365), // Approximate
        }
    }

    /// Calculate retry delay based on priority (higher priority = shorter delay)
    fn calculate_retry_delay(&self, priority: u8) -> u32 {
        match priority {
            1 => 5,   // 5 minutes for highest priority
            2 => 15,  // 15 minutes for high priority
            3 => 30,  // 30 minutes for medium priority
            4 => 60,  // 1 hour for low priority
            _ => 120, // 2 hours for lowest priority
        }
    }

    /// Sort job queue by priority and scheduled time
    fn sort_job_queue(&mut self) {
        let mut jobs: Vec<_> = self.job_queue.drain(..).collect();

        jobs.sort_by(|a, b| {
            // First sort by priority (lower number = higher priority)
            match a.priority.cmp(&b.priority) {
                std::cmp::Ordering::Equal => {
                    // Then sort by scheduled time (earlier = higher priority)
                    a.next_scheduled_crawl.cmp(&b.next_scheduled_crawl)
                }
                other => other,
            }
        });

        self.job_queue = jobs.into();
    }

    /// Get next batch of jobs ready to run
    pub fn get_ready_jobs(&mut self, max_jobs: usize) -> Vec<CrawlJobConfig> {
        let mut ready_jobs = Vec::new();
        let now = Utc::now();

        while ready_jobs.len() < max_jobs && !self.job_queue.is_empty() {
            if let Some(job) = self.job_queue.front() {
                // Check if job is ready to run
                if job.next_scheduled_crawl <= now && self.can_make_request(&job.source) {
                    let job = self.job_queue.pop_front().unwrap();
                    ready_jobs.push(job);
                } else {
                    break; // Jobs are sorted, so if this one isn't ready, none after it are
                }
            }
        }

        ready_jobs
    }

    /// Check if we can make a request to the given source (rate limiting)
    fn can_make_request(&self, source: &DataSource) -> bool {
        if let Some(last_time) = self.last_request_time.get(source) {
            if let Some(rate_limit) = self.rate_limit_per_source.get(source) {
                let min_interval_secs = 60 / (*rate_limit as i64);
                let time_since_last = Utc::now() - *last_time;
                return time_since_last >= Duration::seconds(min_interval_secs);
            }
        }
        true // No previous request or rate limit info
    }

    /// Record that a request was made to a source (for rate limiting)
    pub fn record_request(&mut self, source: DataSource) {
        self.last_request_time.insert(source, Utc::now());
    }

    /// Start a crawl job
    pub fn start_job(&mut self, job: CrawlJobConfig) -> String {
        let job_id = Uuid::new_v4().to_string();
        self.running_jobs.insert(job_id.clone(), job);
        job_id
    }

    /// Complete a crawl job successfully
    pub fn complete_job(&mut self, job_id: &str, records_fetched: usize) -> Result<(), String> {
        if let Some(job) = self.running_jobs.remove(job_id) {
            let now = Utc::now();

            let result = CrawlResult {
                series_id: job.series_id.clone(),
                status: CrawlStatus::Completed,
                records_fetched,
                start_time: now - Duration::minutes(1), // Approximate start time
                end_time: Some(now),
                error_message: None,
                retry_count: 0,
                next_retry: None,
            };

            self.completed_jobs.insert(job.series_id.clone(), result);

            // Schedule next crawl for this series
            let next_crawl = self.calculate_next_crawl_time(&job.frequency, Some(now));
            let mut updated_job = job;
            updated_job.last_successful_crawl = Some(now);
            updated_job.next_scheduled_crawl = next_crawl;

            self.job_queue.push_back(updated_job);
            self.sort_job_queue();

            Ok(())
        } else {
            Err(format!("Job {} not found in running jobs", job_id))
        }
    }

    /// Fail a crawl job and schedule retry if appropriate
    pub fn fail_job(&mut self, job_id: &str, error_message: String) -> Result<(), String> {
        if let Some(mut job) = self.running_jobs.remove(job_id) {
            let now = Utc::now();

            // Get existing retry count from failed jobs if any
            let retry_count = self
                .failed_jobs
                .get(&job.series_id)
                .map(|r| r.retry_count + 1)
                .unwrap_or(1);

            let result = CrawlResult {
                series_id: job.series_id.clone(),
                status: if retry_count <= job.retry_limit {
                    CrawlStatus::Failed
                } else {
                    CrawlStatus::Retrying
                },
                records_fetched: 0,
                start_time: now - Duration::minutes(1), // Approximate start time
                end_time: Some(now),
                error_message: Some(error_message),
                retry_count,
                next_retry: if retry_count <= job.retry_limit {
                    Some(now + Duration::minutes(job.retry_delay_minutes as i64))
                } else {
                    None
                },
            };

            self.failed_jobs
                .insert(job.series_id.clone(), result.clone());

            // Schedule retry if within retry limit
            if retry_count <= job.retry_limit {
                job.next_scheduled_crawl = result.next_retry.unwrap();
                self.job_queue.push_back(job);
                self.sort_job_queue();
            }

            Ok(())
        } else {
            Err(format!("Job {} not found in running jobs", job_id))
        }
    }

    /// Get comprehensive crawler statistics
    pub fn get_stats(&self) -> CrawlerStats {
        let now = Utc::now();
        let today_start = now.date_naive().and_hms_opt(0, 0, 0).unwrap().and_utc();

        let completed_today = self
            .completed_jobs
            .values()
            .filter(|r| r.end_time.map_or(false, |t| t >= today_start))
            .count();

        let failed_today = self
            .failed_jobs
            .values()
            .filter(|r| r.end_time.map_or(false, |t| t >= today_start))
            .count();

        let total_today = completed_today + failed_today;
        let success_rate_24h = if total_today > 0 {
            completed_today as f64 / total_today as f64 * 100.0
        } else {
            0.0
        };

        let average_crawl_time = self
            .completed_jobs
            .values()
            .filter_map(|r| {
                r.end_time
                    .map(|end| (end - r.start_time).num_seconds() as f64)
            })
            .collect::<Vec<f64>>();

        let avg_time = if !average_crawl_time.is_empty() {
            average_crawl_time.iter().sum::<f64>() / average_crawl_time.len() as f64
        } else {
            0.0
        };

        let next_high_priority = self
            .job_queue
            .iter()
            .filter(|job| job.priority <= 2)
            .map(|job| job.next_scheduled_crawl)
            .min();

        // Estimate completion time based on queue size and average crawl time
        let estimated_completion = if !self.job_queue.is_empty() && avg_time > 0.0 {
            let remaining_jobs = self.job_queue.len();
            let estimated_seconds =
                (remaining_jobs as f64 * avg_time) / self.max_concurrent_jobs as f64;
            Some(now + Duration::seconds(estimated_seconds as i64))
        } else {
            None
        };

        CrawlerStats {
            total_series: self.catalog.len(),
            active_series: self.catalog.get_active().len(),
            completed_today,
            failed_today,
            pending_jobs: self.job_queue.len(),
            running_jobs: self.running_jobs.len(),
            average_crawl_time_seconds: avg_time,
            success_rate_24h,
            next_high_priority_job: next_high_priority,
            estimated_completion_time: estimated_completion,
        }
    }

    /// Get series by category for targeted crawling
    pub fn get_jobs_by_category(&self, category: &EconomicCategory) -> Vec<&CrawlJobConfig> {
        self.job_queue
            .iter()
            .filter(|job| job.category == *category)
            .collect()
    }

    /// Get jobs by priority level
    pub fn get_jobs_by_priority(&self, priority: u8) -> Vec<&CrawlJobConfig> {
        self.job_queue
            .iter()
            .filter(|job| job.priority == priority)
            .collect()
    }

    /// Get high-priority jobs that are ready to run
    pub fn get_urgent_jobs(&mut self) -> Vec<CrawlJobConfig> {
        let now = Utc::now();
        let mut urgent_jobs = Vec::new();

        // Get high-priority jobs that are overdue
        while let Some(job) = self.job_queue.front() {
            if job.priority <= 2 && job.next_scheduled_crawl <= now - Duration::hours(1) {
                if self.can_make_request(&job.source) {
                    urgent_jobs.push(self.job_queue.pop_front().unwrap());
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        urgent_jobs
    }

    /// Pause all jobs for a specific source (e.g., during maintenance)
    pub fn pause_source(&mut self, source: &DataSource) {
        // Move jobs back to queue if they're running
        let mut to_reschedule = Vec::new();

        self.running_jobs.retain(|_, job| {
            if job.source == *source {
                to_reschedule.push(job.clone());
                false
            } else {
                true
            }
        });

        // Reschedule paused jobs for later
        for mut job in to_reschedule {
            job.next_scheduled_crawl = Utc::now() + Duration::hours(1); // Delay by 1 hour
            self.job_queue.push_back(job);
        }

        self.sort_job_queue();
    }

    /// Resume jobs for a specific source
    pub fn resume_source(&mut self, source: &DataSource) {
        // Update next scheduled time for paused jobs to now
        for job in &mut self.job_queue {
            if job.source == *source && job.next_scheduled_crawl > Utc::now() {
                job.next_scheduled_crawl = Utc::now();
            }
        }

        self.sort_job_queue();
    }

    /// Get failed jobs that need attention
    pub fn get_failed_jobs(&self) -> Vec<&CrawlResult> {
        self.failed_jobs
            .values()
            .filter(|r| r.retry_count > 3) // Jobs that exceeded retry limit
            .collect()
    }

    /// Reset failed job to retry
    pub fn reset_failed_job(&mut self, series_id: &str) -> Result<(), String> {
        if let Some(failed_result) = self.failed_jobs.remove(series_id) {
            // Find the series definition to recreate the job
            if let Some(series_def) = self.catalog.series.iter().find(|s| s.id == series_id) {
                let job_config = CrawlJobConfig {
                    series_id: series_def.id.clone(),
                    priority: series_def.priority,
                    frequency: series_def.frequency.clone(),
                    source: series_def.source.clone(),
                    category: series_def.category.clone(),
                    is_active: series_def.is_active,
                    retry_limit: 3,
                    retry_delay_minutes: self.calculate_retry_delay(series_def.priority),
                    last_successful_crawl: None,
                    next_scheduled_crawl: Utc::now(), // Schedule immediately
                };

                self.job_queue.push_back(job_config);
                self.sort_job_queue();
                Ok(())
            } else {
                Err(format!("Series definition not found for {}", series_id))
            }
        } else {
            Err(format!("Failed job not found for series {}", series_id))
        }
    }
}

impl Default for EnhancedCrawlerScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_initialization() {
        let mut scheduler = EnhancedCrawlerScheduler::new();
        scheduler.initialize_from_catalog();

        assert!(!scheduler.job_queue.is_empty());
        assert!(scheduler.running_jobs.is_empty());
        assert!(scheduler.completed_jobs.is_empty());
    }

    #[test]
    fn test_priority_sorting() {
        let mut scheduler = EnhancedCrawlerScheduler::new();
        scheduler.initialize_from_catalog();

        // Check that jobs are sorted by priority
        let mut last_priority = 0;
        for job in &scheduler.job_queue {
            assert!(job.priority >= last_priority);
            last_priority = job.priority;
        }
    }

    #[test]
    fn test_rate_limiting() {
        let scheduler = EnhancedCrawlerScheduler::new();

        // Should be able to make first request
        assert!(scheduler.can_make_request(&DataSource::FRED));

        // Test rate limit configuration
        assert!(scheduler
            .rate_limit_per_source
            .contains_key(&DataSource::FRED));
        assert!(scheduler
            .rate_limit_per_source
            .contains_key(&DataSource::BLS));
    }

    #[test]
    fn test_job_lifecycle() {
        let mut scheduler = EnhancedCrawlerScheduler::new();
        scheduler.initialize_from_catalog();

        // Ensure we have jobs in the queue
        assert!(!scheduler.job_queue.is_empty());

        // Get a ready job (may be empty if rate limited, so we'll create a test job)
        let ready_jobs = scheduler.get_ready_jobs(1);

        // If no ready jobs due to rate limiting, create a test job directly
        let job = if ready_jobs.is_empty() {
            // Create a test job that should be ready
            CrawlJobConfig {
                series_id: "TEST_SERIES".to_string(),
                priority: 1,
                frequency: DataFrequency::Monthly,
                source: DataSource::FRED,
                category: EconomicCategory::GDP,
                is_active: true,
                retry_limit: 3,
                retry_delay_minutes: 5,
                last_successful_crawl: None,
                next_scheduled_crawl: Utc::now() - Duration::minutes(1), // Past time, so ready
            }
        } else {
            ready_jobs.into_iter().next().unwrap()
        };

        let job_id = scheduler.start_job(job.clone());

        // Job should be in running jobs
        assert!(scheduler.running_jobs.contains_key(&job_id));

        // Complete the job
        let result = scheduler.complete_job(&job_id, 100);
        assert!(result.is_ok());

        // Job should be completed and rescheduled
        assert!(!scheduler.running_jobs.contains_key(&job_id));
        assert!(scheduler.completed_jobs.contains_key(&job.series_id));
    }

    #[test]
    fn test_retry_logic() {
        let mut scheduler = EnhancedCrawlerScheduler::new();
        scheduler.initialize_from_catalog();

        // Create a test job for retry logic testing
        let job = CrawlJobConfig {
            series_id: "TEST_SERIES_RETRY".to_string(),
            priority: 1,
            frequency: DataFrequency::Monthly,
            source: DataSource::FRED,
            category: EconomicCategory::GDP,
            is_active: true,
            retry_limit: 3,
            retry_delay_minutes: 5,
            last_successful_crawl: None,
            next_scheduled_crawl: Utc::now() - Duration::minutes(1), // Past time, so ready
        };

        let job_id = scheduler.start_job(job.clone());

        // Fail the job
        let result = scheduler.fail_job(&job_id, "Test error".to_string());
        assert!(result.is_ok());

        // Job should be in failed jobs and rescheduled for retry
        assert!(scheduler.failed_jobs.contains_key(&job.series_id));
        assert!(!scheduler.job_queue.is_empty()); // Should be rescheduled
    }

    #[test]
    fn test_stats_calculation() {
        let scheduler = EnhancedCrawlerScheduler::new();
        let stats = scheduler.get_stats();

        assert!(stats.total_series > 0);
        assert!(stats.active_series > 0);
        assert_eq!(stats.pending_jobs, 0); // No jobs initialized yet
        assert_eq!(stats.running_jobs, 0);
    }

    #[test]
    fn test_category_filtering() {
        let mut scheduler = EnhancedCrawlerScheduler::new();
        scheduler.initialize_from_catalog();

        let gdp_jobs = scheduler.get_jobs_by_category(&EconomicCategory::GDP);
        assert!(!gdp_jobs.is_empty());

        let employment_jobs = scheduler.get_jobs_by_category(&EconomicCategory::Employment);
        assert!(!employment_jobs.is_empty());
    }
}
