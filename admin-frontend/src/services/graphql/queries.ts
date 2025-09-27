/**
 * GraphQL queries for crawler administration
 *
 * Defines all queries used by the admin interface:
 * - Crawler status and monitoring
 * - Queue statistics and management
 * - Data source configuration
 * - Performance metrics and logs
 */

import { gql } from "@apollo/client";

// ============================================================================
// CRAWLER STATUS QUERIES
// ============================================================================

export const GET_CRAWLER_STATUS = gql`
  query GetCrawlerStatus {
    crawlerStatus {
      is_running
      active_workers
      last_crawl
      next_scheduled_crawl
    }
  }
`;

export const GET_QUEUE_STATISTICS = gql`
  query GetQueueStatistics {
    queueStatistics {
      total_items
      pending_items
      processing_items
      completed_items
      failed_items
      retrying_items
      oldest_pending
      average_processing_time
    }
  }
`;

// ============================================================================
// DATA SOURCE QUERIES
// ============================================================================

export const GET_DATA_SOURCES = gql`
  query GetDataSources {
    dataSources {
      id
      name
      description
      base_url
      api_key_required
      rate_limit_per_minute
      is_active
      last_health_check
      health_status
      created_at
      updated_at
    }
  }
`;

export const GET_DATA_SOURCE_BY_ID = gql`
  query GetDataSourceById($id: ID!) {
    dataSource(id: $id) {
      id
      name
      description
      base_url
      api_key_required
      rate_limit_per_minute
      is_active
      last_health_check
      health_status
      configuration
      created_at
      updated_at
    }
  }
`;

// ============================================================================
// CRAWLER CONFIGURATION QUERIES
// ============================================================================

export const GET_CRAWLER_CONFIG = gql`
  query GetCrawlerConfig {
    crawlerConfig {
      max_workers
      queue_size_limit
      default_timeout_seconds
      default_retry_attempts
      schedule_frequency
      maintenance_mode
      global_rate_limit
      created_at
      updated_at
    }
  }
`;

// ============================================================================
// LOGS AND MONITORING QUERIES
// ============================================================================

export const GET_CRAWLER_LOGS = gql`
  query GetCrawlerLogs(
    $limit: Int
    $offset: Int
    $level: String
    $source: String
    $search: String
    $start_date: String
    $end_date: String
  ) {
    crawlerLogs(
      limit: $limit
      offset: $offset
      level: $level
      source: $source
      search: $search
      start_date: $start_date
      end_date: $end_date
    ) {
      id
      timestamp
      level
      source
      message
      details
      duration_ms
      status
      created_at
    }
  }
`;

export const GET_PERFORMANCE_METRICS = gql`
  query GetPerformanceMetrics($time_range: String) {
    performanceMetrics(time_range: $time_range) {
      cpu_usage_percent
      memory_usage_percent
      queue_depth
      error_rate_percent
      throughput_per_minute
      average_response_time_ms
      timestamp
    }
  }
`;

// ============================================================================
// SYSTEM HEALTH QUERIES
// ============================================================================

export const GET_SYSTEM_HEALTH = gql`
  query GetSystemHealth {
    systemHealth {
      overall_status
      components {
        name
        status
        last_check
        message
      }
      uptime_seconds
      version
      environment
    }
  }
`;

// ============================================================================
// SEARCH AND FILTER QUERIES
// ============================================================================

export const SEARCH_LOGS = gql`
  query SearchLogs($query: String!, $filters: LogFilters) {
    searchLogs(query: $query, filters: $filters) {
      id
      timestamp
      level
      source
      message
      details
      duration_ms
      status
      relevance_score
    }
  }
`;

export const GET_LOG_STATISTICS = gql`
  query GetLogStatistics($time_range: String) {
    logStatistics(time_range: $time_range) {
      total_logs
      logs_by_level {
        level
        count
      }
      logs_by_source {
        source
        count
      }
      error_rate_percent
      average_duration_ms
    }
  }
`;

// ============================================================================
// PAGINATION AND COUNTING QUERIES
// ============================================================================

export const GET_LOGS_COUNT = gql`
  query GetLogsCount($filters: LogFilters) {
    logsCount(filters: $filters)
  }
`;

export const GET_QUEUE_ITEMS = gql`
  query GetQueueItems(
    $limit: Int
    $offset: Int
    $status: String
    $source: String
  ) {
    queueItems(
      limit: $limit
      offset: $offset
      status: $status
      source: $source
    ) {
      id
      source
      series_id
      priority
      status
      created_at
      started_at
      completed_at
      error_message
      retry_count
    }
  }
`;

// ============================================================================
// REAL-TIME SUBSCRIPTIONS (for future WebSocket implementation)
// ============================================================================

export const CRAWLER_STATUS_SUBSCRIPTION = gql`
  subscription CrawlerStatusSubscription {
    crawlerStatusUpdated {
      is_running
      active_workers
      last_crawl
      next_scheduled_crawl
    }
  }
`;

export const QUEUE_STATISTICS_SUBSCRIPTION = gql`
  subscription QueueStatisticsSubscription {
    queueStatisticsUpdated {
      total_items
      pending_items
      processing_items
      completed_items
      failed_items
      retrying_items
      oldest_pending
      average_processing_time
    }
  }
`;

export const LOG_ENTRY_SUBSCRIPTION = gql`
  subscription LogEntrySubscription {
    newLogEntry {
      id
      timestamp
      level
      source
      message
      details
      duration_ms
      status
    }
  }
`;

// ============================================================================
// TYPE DEFINITIONS FOR TYPESCRIPT
// ============================================================================

export interface CrawlerStatus {
  is_running: boolean;
  active_workers: number;
  last_crawl?: string;
  next_scheduled_crawl?: string;
}

export interface QueueStatistics {
  total_items: number;
  pending_items: number;
  processing_items: number;
  completed_items: number;
  failed_items: number;
  retrying_items: number;
  oldest_pending?: string;
  average_processing_time?: number;
}

export interface DataSource {
  id: string;
  name: string;
  description: string;
  base_url: string;
  api_key_required: boolean;
  rate_limit_per_minute: number;
  is_active: boolean;
  last_health_check?: string;
  health_status: string;
  created_at: string;
  updated_at: string;
}

export interface CrawlerConfig {
  max_workers: number;
  queue_size_limit: number;
  default_timeout_seconds: number;
  default_retry_attempts: number;
  schedule_frequency: string;
  maintenance_mode: boolean;
  global_rate_limit: number;
  created_at: string;
  updated_at: string;
}

export interface LogEntry {
  id: string;
  timestamp: string;
  level: string;
  source: string;
  message: string;
  details?: string;
  duration_ms?: number;
  status: string;
  created_at: string;
}

export interface PerformanceMetrics {
  cpu_usage_percent: number;
  memory_usage_percent: number;
  queue_depth: number;
  error_rate_percent: number;
  throughput_per_minute: number;
  average_response_time_ms: number;
  timestamp: string;
}

export interface SystemHealth {
  overall_status: string;
  components: Array<{
    name: string;
    status: string;
    last_check: string;
    message?: string;
  }>;
  uptime_seconds: number;
  version: string;
  environment: string;
}
