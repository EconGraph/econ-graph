/**
 * GraphQL mutations for crawler administration
 *
 * Defines all mutations used by the admin interface:
 * - Crawler control operations (start, stop, pause, resume)
 * - Configuration management
 * - Data source management
 * - Queue operations
 */

import { gql } from "@apollo/client";

// ============================================================================
// CRAWLER CONTROL MUTATIONS
// ============================================================================

export const TRIGGER_CRAWL = gql`
  mutation TriggerCrawl($input: TriggerCrawlInput!) {
    triggerCrawl(input: $input) {
      is_running
      active_workers
      last_crawl
      next_scheduled_crawl
    }
  }
`;

export const START_CRAWLER = gql`
  mutation StartCrawler {
    startCrawler {
      success
      message
      crawlerStatus {
        is_running
        active_workers
        last_crawl
        next_scheduled_crawl
      }
    }
  }
`;

export const STOP_CRAWLER = gql`
  mutation StopCrawler {
    stopCrawler {
      success
      message
      crawlerStatus {
        is_running
        active_workers
        last_crawl
        next_scheduled_crawl
      }
    }
  }
`;

export const PAUSE_CRAWLER = gql`
  mutation PauseCrawler {
    pauseCrawler {
      success
      message
      crawlerStatus {
        is_running
        active_workers
        last_crawl
        next_scheduled_crawl
      }
    }
  }
`;

export const RESUME_CRAWLER = gql`
  mutation ResumeCrawler {
    resumeCrawler {
      success
      message
      crawlerStatus {
        is_running
        active_workers
        last_crawl
        next_scheduled_crawl
      }
    }
  }
`;

// ============================================================================
// CONFIGURATION MUTATIONS
// ============================================================================

export const UPDATE_CRAWLER_CONFIG = gql`
  mutation UpdateCrawlerConfig($input: CrawlerConfigInput!) {
    updateCrawlerConfig(input: $input) {
      success
      message
      config {
        max_workers
        queue_size_limit
        default_timeout_seconds
        default_retry_attempts
        schedule_frequency
        maintenance_mode
        global_rate_limit
        updated_at
      }
    }
  }
`;

export const SET_MAINTENANCE_MODE = gql`
  mutation SetMaintenanceMode($enabled: Boolean!) {
    setMaintenanceMode(enabled: $enabled) {
      success
      message
      maintenance_mode
    }
  }
`;

// ============================================================================
// DATA SOURCE MUTATIONS
// ============================================================================

export const CREATE_DATA_SOURCE = gql`
  mutation CreateDataSource($input: DataSourceInput!) {
    createDataSource(input: $input) {
      success
      message
      dataSource {
        id
        name
        description
        base_url
        api_key_required
        rate_limit_per_minute
        is_active
        health_status
        created_at
      }
    }
  }
`;

export const UPDATE_DATA_SOURCE = gql`
  mutation UpdateDataSource($id: ID!, $input: DataSourceInput!) {
    updateDataSource(id: $id, input: $input) {
      success
      message
      dataSource {
        id
        name
        description
        base_url
        api_key_required
        rate_limit_per_minute
        is_active
        health_status
        updated_at
      }
    }
  }
`;

export const DELETE_DATA_SOURCE = gql`
  mutation DeleteDataSource($id: ID!) {
    deleteDataSource(id: $id) {
      success
      message
    }
  }
`;

export const TOGGLE_DATA_SOURCE = gql`
  mutation ToggleDataSource($id: ID!, $enabled: Boolean!) {
    toggleDataSource(id: $id, enabled: $enabled) {
      success
      message
      dataSource {
        id
        is_active
        updated_at
      }
    }
  }
`;

export const TEST_DATA_SOURCE_CONNECTION = gql`
  mutation TestDataSourceConnection($id: ID!) {
    testDataSourceConnection(id: $id) {
      success
      message
      connection_status
      response_time_ms
      last_health_check
    }
  }
`;

// ============================================================================
// QUEUE MANAGEMENT MUTATIONS
// ============================================================================

export const CLEAR_QUEUE = gql`
  mutation ClearQueue($status: String) {
    clearQueue(status: $status) {
      success
      message
      cleared_count
    }
  }
`;

export const RETRY_FAILED_ITEMS = gql`
  mutation RetryFailedItems($limit: Int) {
    retryFailedItems(limit: $limit) {
      success
      message
      retried_count
    }
  }
`;

export const PRIORITIZE_QUEUE_ITEM = gql`
  mutation PrioritizeQueueItem($id: ID!, $priority: Int!) {
    prioritizeQueueItem(id: $id, priority: $priority) {
      success
      message
      queueItem {
        id
        priority
        updated_at
      }
    }
  }
`;

export const CANCEL_QUEUE_ITEM = gql`
  mutation CancelQueueItem($id: ID!) {
    cancelQueueItem(id: $id) {
      success
      message
    }
  }
`;

// ============================================================================
// LOG MANAGEMENT MUTATIONS
// ============================================================================

export const CLEAR_LOGS = gql`
  mutation ClearLogs($filters: LogFilters) {
    clearLogs(filters: $filters) {
      success
      message
      cleared_count
    }
  }
`;

export const EXPORT_LOGS = gql`
  mutation ExportLogs($filters: LogFilters, $format: String!) {
    exportLogs(filters: $filters, format: $format) {
      success
      message
      download_url
      file_size
    }
  }
`;

// ============================================================================
// SYSTEM MANAGEMENT MUTATIONS
// ============================================================================

export const RESTART_SYSTEM = gql`
  mutation RestartSystem {
    restartSystem {
      success
      message
      restart_scheduled_at
    }
  }
`;

export const BACKUP_SYSTEM = gql`
  mutation BackupSystem {
    backupSystem {
      success
      message
      backup_id
      download_url
    }
  }
`;

// ============================================================================
// LOG SEARCH MUTATION
// ============================================================================

export const SEARCH_LOGS = gql`
  mutation SearchLogs($query: String!, $filters: LogFilters) {
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

// ============================================================================
// INPUT TYPE DEFINITIONS
// ============================================================================

export interface TriggerCrawlInput {
  sources?: string[];
  series_ids?: string[];
  priority?: number;
  force?: boolean;
}

export interface CrawlerConfigInput {
  max_workers?: number;
  queue_size_limit?: number;
  default_timeout_seconds?: number;
  default_retry_attempts?: number;
  schedule_frequency?: string;
  maintenance_mode?: boolean;
  global_rate_limit?: number;
}

export interface DataSourceInput {
  name: string;
  description?: string;
  base_url: string;
  api_key_required?: boolean;
  rate_limit_per_minute?: number;
  is_active?: boolean;
  configuration?: Record<string, any>;
}

export interface LogFilters {
  level?: string;
  source?: string;
  search?: string;
  start_date?: string;
  end_date?: string;
  status?: string;
}

// ============================================================================
// RESPONSE TYPE DEFINITIONS
// ============================================================================

export interface MutationResponse {
  success: boolean;
  message: string;
}

export interface CrawlerControlResponse extends MutationResponse {
  crawlerStatus?: {
    is_running: boolean;
    active_workers: number;
    last_crawl?: string;
    next_scheduled_crawl?: string;
  };
}

export interface ConfigUpdateResponse extends MutationResponse {
  config?: {
    max_workers: number;
    queue_size_limit: number;
    default_timeout_seconds: number;
    default_retry_attempts: number;
    schedule_frequency: string;
    maintenance_mode: boolean;
    global_rate_limit: number;
    updated_at: string;
  };
}

export interface DataSourceResponse extends MutationResponse {
  dataSource?: {
    id: string;
    name: string;
    description: string;
    base_url: string;
    api_key_required: boolean;
    rate_limit_per_minute: number;
    is_active: boolean;
    health_status: string;
    created_at?: string;
    updated_at?: string;
  };
}

export interface ConnectionTestResponse extends MutationResponse {
  connection_status: string;
  response_time_ms?: number;
  last_health_check?: string;
}

export interface QueueOperationResponse extends MutationResponse {
  cleared_count?: number;
  retried_count?: number;
  queueItem?: {
    id: string;
    priority?: number;
    updated_at?: string;
  };
}

export interface LogOperationResponse extends MutationResponse {
  cleared_count?: number;
  download_url?: string;
  file_size?: number;
}

export interface SystemOperationResponse extends MutationResponse {
  restart_scheduled_at?: string;
  backup_id?: string;
  download_url?: string;
}
