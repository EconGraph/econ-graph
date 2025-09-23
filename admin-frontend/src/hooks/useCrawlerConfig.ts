/**
 * Custom hooks for crawler configuration management
 *
 * Provides:
 * - Data source configuration management
 * - Global crawler settings
 * - Configuration validation and testing
 * - Real-time configuration updates
 */

import { useQuery, useMutation } from "@apollo/client/react";
import { useCallback, useState } from "react";
import {
  GET_CRAWLER_CONFIG,
  GET_DATA_SOURCES,
  GET_DATA_SOURCE_BY_ID,
  type CrawlerConfig,
  type DataSource,
} from "../services/graphql/queries";
import {
  UPDATE_CRAWLER_CONFIG,
  CREATE_DATA_SOURCE,
  UPDATE_DATA_SOURCE,
  DELETE_DATA_SOURCE,
  TOGGLE_DATA_SOURCE,
  TEST_DATA_SOURCE_CONNECTION,
  SET_MAINTENANCE_MODE,
  type CrawlerConfigInput,
  type DataSourceInput,
} from "../services/graphql/mutations";

// ============================================================================
// CRAWLER CONFIGURATION HOOK
// ============================================================================

export const useCrawlerConfig = () => {
  const { data, loading, error, refetch } = useQuery<{
    crawlerConfig: CrawlerConfig;
  }>(GET_CRAWLER_CONFIG, {
    errorPolicy: "all",
  });

  const [updateConfig] = useMutation(UPDATE_CRAWLER_CONFIG, {
    errorPolicy: "all",
  });
  const [setMaintenanceMode] = useMutation(SET_MAINTENANCE_MODE, {
    errorPolicy: "all",
  });

  const [updateLoading, setUpdateLoading] = useState(false);
  const [updateError, setUpdateError] = useState<string | null>(null);

  const refresh = useCallback(() => {
    refetch();
  }, [refetch]);

  const updateConfiguration = useCallback(
    async (input: CrawlerConfigInput) => {
      try {
        setUpdateLoading(true);
        setUpdateError(null);
        const result = await updateConfig({ variables: { input } });
        await refetch(); // Refresh the config after update
        return (result.data as any)?.updateCrawlerConfig;
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to update configuration";
        setUpdateError(errorMessage);
        throw err;
      } finally {
        setUpdateLoading(false);
      }
    },
    [updateConfig, refetch],
  );

  const toggleMaintenanceMode = useCallback(
    async (enabled: boolean) => {
      try {
        setUpdateLoading(true);
        setUpdateError(null);
        const result = await setMaintenanceMode({ variables: { enabled } });
        await refetch(); // Refresh the config after update
        return (result.data as any)?.setMaintenanceMode;
      } catch (err) {
        const errorMessage =
          err instanceof Error
            ? err.message
            : "Failed to toggle maintenance mode";
        setUpdateError(errorMessage);
        throw err;
      } finally {
        setUpdateLoading(false);
      }
    },
    [setMaintenanceMode, refetch],
  );

  return {
    config: data?.crawlerConfig,
    loading: loading || updateLoading,
    error: error || updateError,
    refresh,
    updateConfiguration,
    toggleMaintenanceMode,
  };
};

// ============================================================================
// DATA SOURCES HOOK
// ============================================================================

export const useDataSources = () => {
  const { data, loading, error, refetch } = useQuery<{
    dataSources: DataSource[];
  }>(GET_DATA_SOURCES, {
    errorPolicy: "all",
  });

  const [createDataSource] = useMutation(CREATE_DATA_SOURCE);
  const [updateDataSource] = useMutation(UPDATE_DATA_SOURCE);
  const [deleteDataSource] = useMutation(DELETE_DATA_SOURCE);
  const [toggleDataSource] = useMutation(TOGGLE_DATA_SOURCE);
  const [testConnection] = useMutation(TEST_DATA_SOURCE_CONNECTION);

  const [operationLoading, setOperationLoading] = useState(false);
  const [operationError, setOperationError] = useState<string | null>(null);

  const refresh = useCallback(() => {
    refetch();
  }, [refetch]);

  const createNewDataSource = useCallback(
    async (input: DataSourceInput) => {
      try {
        setOperationLoading(true);
        setOperationError(null);
        const result = await createDataSource({ variables: { input } });
        await refetch(); // Refresh the list after creation
        return (result.data as any)?.createDataSource;
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to create data source";
        setOperationError(errorMessage);
        throw err;
      } finally {
        setOperationLoading(false);
      }
    },
    [createDataSource, refetch],
  );

  const updateExistingDataSource = useCallback(
    async (id: string, input: DataSourceInput) => {
      try {
        setOperationLoading(true);
        setOperationError(null);
        const result = await updateDataSource({ variables: { id, input } });
        await refetch(); // Refresh the list after update
        return (result.data as any)?.updateDataSource;
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to update data source";
        setOperationError(errorMessage);
        throw err;
      } finally {
        setOperationLoading(false);
      }
    },
    [updateDataSource, refetch],
  );

  const removeDataSource = useCallback(
    async (id: string) => {
      try {
        setOperationLoading(true);
        setOperationError(null);
        const result = await deleteDataSource({ variables: { id } });
        await refetch(); // Refresh the list after deletion
        return (result.data as any)?.deleteDataSource;
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to delete data source";
        setOperationError(errorMessage);
        throw err;
      } finally {
        setOperationLoading(false);
      }
    },
    [deleteDataSource, refetch],
  );

  const toggleDataSourceStatus = useCallback(
    async (id: string, enabled: boolean) => {
      try {
        setOperationLoading(true);
        setOperationError(null);
        const result = await toggleDataSource({ variables: { id, enabled } });
        await refetch(); // Refresh the list after toggle
        return (result.data as any)?.toggleDataSource;
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to toggle data source";
        setOperationError(errorMessage);
        throw err;
      } finally {
        setOperationLoading(false);
      }
    },
    [toggleDataSource, refetch],
  );

  const testDataSourceConnection = useCallback(
    async (id: string) => {
      try {
        setOperationLoading(true);
        setOperationError(null);
        const result = await testConnection({ variables: { id } });
        return (result.data as any)?.testDataSourceConnection;
      } catch (err) {
        const errorMessage =
          err instanceof Error ? err.message : "Failed to test connection";
        setOperationError(errorMessage);
        throw err;
      } finally {
        setOperationLoading(false);
      }
    },
    [testConnection],
  );

  return {
    dataSources: data?.dataSources || [],
    loading: loading || operationLoading,
    error: error || operationError,
    refresh,
    actions: {
      create: createNewDataSource,
      update: updateExistingDataSource,
      delete: removeDataSource,
      toggle: toggleDataSourceStatus,
      testConnection: testDataSourceConnection,
    },
  };
};

// ============================================================================
// SINGLE DATA SOURCE HOOK
// ============================================================================

export const useDataSource = (id: string) => {
  const { data, loading, error, refetch } = useQuery<{
    dataSource: DataSource;
  }>(GET_DATA_SOURCE_BY_ID, {
    variables: { id },
    errorPolicy: "all",
    skip: !id,
  });

  const refresh = useCallback(() => {
    if (id) {
      refetch();
    }
  }, [id, refetch]);

  return {
    dataSource: data?.dataSource,
    loading,
    error,
    refresh,
  };
};

// ============================================================================
// COMBINED CONFIGURATION HOOK
// ============================================================================

export const useCrawlerConfiguration = () => {
  const config = useCrawlerConfig();
  const dataSources = useDataSources();

  const refreshAll = useCallback(() => {
    config.refresh();
    dataSources.refresh();
  }, [config, dataSources]);

  return {
    config,
    dataSources,
    refreshAll,
    loading: config.loading || dataSources.loading,
    error: config.error || dataSources.error,
  };
};
