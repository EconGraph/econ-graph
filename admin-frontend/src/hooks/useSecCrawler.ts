/**
 * SEC Crawler Hook
 *
 * Provides GraphQL integration for SEC crawler operations.
 * Includes company crawling, RSS import, and status monitoring.
 */

import { useState, useCallback } from "react";
import { useMutation } from "@apollo/client/react";
import { gql } from "@apollo/client";

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
  crawlCompany: (input: SecCrawlInput) => Promise<SecCrawlResult>;
  importRssFeed: (input: SecRssImportInput) => Promise<SecRssImportResult>;
  isCrawling: boolean;
  progress: number;
  status: string;
  error?: Error;
}

export const useSecCrawler = (): UseSecCrawlerReturn => {
  const [isCrawling, setIsCrawling] = useState(false);
  const [progress, setProgress] = useState(0);
  const [status, setStatus] = useState("idle");

  // Mutation for triggering SEC crawl
  const [triggerCrawlMutation, { error: crawlError }] = useMutation<{
    triggerSecCrawl: SecCrawlResult;
  }>(TRIGGER_SEC_CRAWL);

  // Mutation for importing RSS feed
  const [importRssMutation, { error: rssError }] = useMutation<{
    importSecRss: SecRssImportResult;
  }>(IMPORT_SEC_RSS);

  // Trigger SEC crawl
  const crawlCompany = useCallback(
    async (input: SecCrawlInput): Promise<SecCrawlResult> => {
      try {
        setIsCrawling(true);
        setStatus("crawling");
        setProgress(0);

        const result = await triggerCrawlMutation({
          variables: { input },
        });

        if (!result.data?.triggerSecCrawl) {
          throw new Error("No crawl result returned");
        }

        setProgress(100);
        setStatus("completed");
        setIsCrawling(false);

        return result.data.triggerSecCrawl;
      } catch (error) {
        console.error("Error triggering SEC crawl:", error);
        setStatus("error");
        setIsCrawling(false);
        throw error;
      }
    },
    [triggerCrawlMutation],
  );

  // Import RSS feed
  const importRssFeed = useCallback(
    async (input: SecRssImportInput): Promise<SecRssImportResult> => {
      try {
        const result = await importRssMutation({
          variables: { input },
        });

        if (!result.data?.importSecRss) {
          throw new Error("No RSS import result returned");
        }

        return result.data.importSecRss;
      } catch (error) {
        console.error("Error importing RSS feed:", error);
        throw error;
      }
    },
    [importRssMutation],
  );

  return {
    crawlCompany,
    importRssFeed,
    isCrawling,
    progress,
    status,
    error: crawlError || rssError,
  };
};

export default useSecCrawler;
