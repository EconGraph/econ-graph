/** Poll the executable crawler contract. Mutations enqueue work; they never control processes. */
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  requestGraphQL,
  getAuthSessionSnapshot,
  subscribeAuthSession,
  isAuthSessionReady,
  getAuthSessionError,
  assertAuthSessionReady,
} from "../services/graphqlClient";
import { useSyncExternalStore } from "react";

export interface CrawlerSource {
  source: string;
  name: string;
  enabled: boolean | null;
  crawlFrequencyHours: number | null;
  requiresAdminApproval: boolean | null;
}
export interface CrawlerJob {
  id: string;
  source: string;
  seriesId: string;
  kind: string;
  status: string;
  priority: number;
  retryCount: number;
  maxRetries: number;
  scheduledFor: string | null;
  startedAt: string | null;
  finishedAt: string | null;
  updatedAt: string;
  errorMessage: string | null;
}
export interface Overview {
  crawlerStatus: {
    isRunning: boolean;
    activeWorkers: number;
    lastCrawl: string | null;
    nextScheduledCrawl: string | null;
    sources: {
      source: string;
      pending: number;
      processing: number;
      retrying: number;
      failedLastDay: number;
      lastSuccess: string | null;
    }[];
  };
  queueStatistics: {
    totalItems: number;
    pendingItems: number;
    processingItems: number;
    completedItems: number;
    failedItems: number;
    retryingItems: number;
  };
  crawlerSources: CrawlerSource[];
}

export const OVERVIEW = `query CrawlerAdminOverview {
  crawlerStatus { isRunning activeWorkers lastCrawl nextScheduledCrawl sources { source pending processing retrying failedLastDay lastSuccess } }
  queueStatistics { totalItems pendingItems processingItems completedItems failedItems retryingItems }
  crawlerSources { source name enabled crawlFrequencyHours requiresAdminApproval }
}`;
export const JOBS = `query CrawlerAdminJobs($limit:Int!, $offset:Int!, $source:String, $status:String) {
  crawlerJobs(limit:$limit, offset:$offset, source:$source, status:$status) {
    id source seriesId kind status priority retryCount maxRetries scheduledFor startedAt finishedAt updatedAt errorMessage
  }
}`;

export async function crawlerRequest<T>(
  query: string,
  variables?: Record<string, unknown>,
): Promise<T> {
  assertAuthSessionReady();
  return requestGraphQL<T>(query, variables, { retry: false });
}

export function useCrawlerOverview() {
  const session = useSyncExternalStore(
    subscribeAuthSession,
    getAuthSessionSnapshot,
  );
  const query = useQuery({
    queryKey: ["crawlerAdmin", session, "overview"],
    enabled: isAuthSessionReady(),
    queryFn: async () => {
      const snapshot = await crawlerRequest<Overview>(OVERVIEW);
      const counts = snapshot.queueStatistics;
      if (
        !snapshot.crawlerStatus ||
        !counts ||
        !Array.isArray(snapshot.crawlerSources) ||
        [
          snapshot.crawlerStatus.activeWorkers,
          counts.pendingItems,
          counts.processingItems,
          counts.completedItems,
          counts.failedItems,
          counts.retryingItems,
        ].some((value) => !Number.isInteger(value) || value < 0)
      ) {
        throw new Error("Crawler service returned no valid queue snapshot");
      }
      return snapshot;
    },
    refetchInterval: 15_000,
    refetchIntervalInBackground: false,
    retry: false,
  });
  return { ...query, error: getAuthSessionError() ?? query.error };
}
export function useCrawlerJobs(
  source: string,
  status: string,
  offset: number,
  polling: boolean,
) {
  const session = useSyncExternalStore(
    subscribeAuthSession,
    getAuthSessionSnapshot,
  );
  const query = useQuery({
    queryKey: ["crawlerAdmin", session, "jobs", source, status, offset],
    enabled: isAuthSessionReady(),
    queryFn: async () => {
      const result = await crawlerRequest<{ crawlerJobs: CrawlerJob[] }>(JOBS, {
        limit: 50,
        offset,
        source: source || null,
        status: status || null,
      });
      if (!Array.isArray(result.crawlerJobs))
        throw new Error("Crawler service returned no execution history");
      return result;
    },
    refetchInterval: polling ? 10_000 : false,
    refetchIntervalInBackground: false,
    retry: false,
  });
  return { ...query, error: getAuthSessionError() ?? query.error };
}
export function useCrawlerActions() {
  const session = useSyncExternalStore(
    subscribeAuthSession,
    getAuthSessionSnapshot,
  );
  const client = useQueryClient();
  const refresh = () =>
    client.invalidateQueries({ queryKey: ["crawlerAdmin"] });
  const enqueue = useMutation({
    mutationKey: ["crawlerAdmin", session, "enqueue"],
    retry: false,
    mutationFn: (input: {
      source: string;
      seriesIds: string[];
      priority: number;
    }) =>
      crawlerRequest<{ triggerCrawl: { enqueuedCount: number } }>(
        `mutation CrawlerAdminEnqueue($input:TriggerCrawlInput!) { triggerCrawl(input:$input) { enqueuedCount } }`,
        { input },
      ),
    onSuccess: refresh,
  });
  const schedule = useMutation({
    mutationKey: ["crawlerAdmin", session, "schedule"],
    retry: false,
    mutationFn: (variables: { source: string; enabled: boolean }) =>
      crawlerRequest<{ setCrawlerSourceEnabled: boolean }>(
        `mutation CrawlerAdminSchedule($source:String!, $enabled:Boolean!) { setCrawlerSourceEnabled(source:$source,enabled:$enabled) }`,
        variables,
      ),
    onSuccess: refresh,
  });
  return { enqueue, schedule };
}
