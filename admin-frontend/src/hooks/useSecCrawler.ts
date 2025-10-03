/**
 * SEC Crawler Hook
 * 
 * Provides GraphQL integration for SEC crawler operations.
 * Includes company crawling, RSS import, and status monitoring.
 */

import { useState, useCallback } from 'react';
import { useMutation } from '@apollo/client';
import { gql } from '@apollo/client';

// GraphQL mutation for triggering SEC crawl
const TRIGGER_SEC_CRAWL = gql`
  mutation TriggerSecCrawl($input: SecCrawlInput!) {
    triggerSecCrawl(input: $input) {
      operation_id
      cik
      filings_downloaded
      filings_processed
      errors
      start_time
      end_time
      status
    }
  }
`;

// GraphQL mutation for importing SEC RSS feed
const IMPORT_SEC_RSS = gql`
  mutation ImportSecRss($input: SecRssImportInput!) {
    importSecRss(input: $input) {
      operation_id
      filings_imported
      companies_added
      errors
      start_time
      end_time
      status
    }
  }
`;

export interface SecCrawlInput {
  cik: string;
  form_types?: string;
  start_date?: string;
  end_date?: string;
  exclude_amended?: boolean;
  exclude_restated?: boolean;
  max_file_size?: number;
}

export interface SecRssImportInput {
  rss_url?: string;
  max_filings?: number;
  form_types?: string;
}

export interface SecCrawlResult {
  operation_id: string;
  cik: string;
  filings_downloaded: number;
  filings_processed: number;
  errors: number;
  start_time: string;
  end_time?: string;
  status: string;
}

export interface SecRssImportResult {
  operation_id: string;
  filings_imported: number;
  companies_added: number;
  errors: number;
  start_time: string;
  end_time?: string;
  status: string;
}

export interface UseSecCrawlerReturn {
  crawlResults: SecCrawlResult[];
  rssResults: SecRssImportResult[];
  loading: boolean;
  error?: Error;
  triggerCrawl: (input: SecCrawlInput) => Promise<SecCrawlResult>;
  importRss: (input: SecRssImportInput) => Promise<SecRssImportResult>;
  clearResults: () => void;
}

export const useSecCrawler = (): UseSecCrawlerReturn => {
  const [crawlResults, setCrawlResults] = useState<SecCrawlResult[]>([]);
  const [rssResults, setRssResults] = useState<SecRssImportResult[]>([]);

  // Mutation for triggering SEC crawl
  const [triggerCrawlMutation, { loading: crawlLoading, error: crawlError }] = useMutation<{
    triggerSecCrawl: SecCrawlResult;
  }>(TRIGGER_SEC_CRAWL, {
    onCompleted: (data) => {
      if (data.triggerSecCrawl) {
        setCrawlResults(prev => [data.triggerSecCrawl, ...prev]);
      }
    },
    onError: (error) => {
      console.error('SEC crawl error:', error);
    },
  });

  // Mutation for importing RSS feed
  const [importRssMutation, { loading: rssLoading, error: rssError }] = useMutation<{
    importSecRss: SecRssImportResult;
  }>(IMPORT_SEC_RSS, {
    onCompleted: (data) => {
      if (data.importSecRss) {
        setRssResults(prev => [data.importSecRss, ...prev]);
      }
    },
    onError: (error) => {
      console.error('RSS import error:', error);
    },
  });

  // Trigger SEC crawl
  const triggerCrawl = useCallback(
    async (input: SecCrawlInput): Promise<SecCrawlResult> => {
      try {
        const result = await triggerCrawlMutation({
          variables: { input },
        });
        
        if (!result.data?.triggerSecCrawl) {
          throw new Error('No crawl result returned');
        }
        
        return result.data.triggerSecCrawl;
      } catch (error) {
        console.error('Error triggering SEC crawl:', error);
        throw error;
      }
    },
    [triggerCrawlMutation]
  );

  // Import RSS feed
  const importRss = useCallback(
    async (input: SecRssImportInput): Promise<SecRssImportResult> => {
      try {
        const result = await importRssMutation({
          variables: { input },
        });
        
        if (!result.data?.importSecRss) {
          throw new Error('No RSS import result returned');
        }
        
        return result.data.importSecRss;
      } catch (error) {
        console.error('Error importing RSS feed:', error);
        throw error;
      }
    },
    [importRssMutation]
  );

  // Clear results
  const clearResults = useCallback(() => {
    setCrawlResults([]);
    setRssResults([]);
  }, []);

  return {
    crawlResults,
    rssResults,
    loading: crawlLoading || rssLoading,
    error: crawlError || rssError,
    triggerCrawl,
    importRss,
    clearResults,
  };
};

export default useSecCrawler;
