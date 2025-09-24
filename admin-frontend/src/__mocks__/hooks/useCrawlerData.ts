/**
 * Manual mock for useCrawlerData hook
 * This file is automatically picked up by Jest when the module is imported
 */

export const useCrawlerData = jest.fn(() => ({
  status: {
    status: {
      is_running: true,
      active_workers: 5,
      last_crawl: new Date().toISOString(),
      next_scheduled_crawl: new Date(Date.now() + 60000).toISOString(),
    },
  },
  queueStats: {
    statistics: {
      total_items: 1000,
      pending_items: 25,
      completed_items: 950,
      failed_items: 25,
      processing_rate: 15.5,
      average_processing_time: 2.3,
    },
  },
  control: {
    startCrawler: jest.fn(),
    stopCrawler: jest.fn(),
    pauseCrawler: jest.fn(),
    resumeCrawler: jest.fn(),
  },
  refreshAll: jest.fn(),
  loading: false,
  error: null,
}));
