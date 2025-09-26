// REQUIREMENT: Validate all GraphQL mock files conform to backend schema
// PURPOSE: Ensure mock data matches actual GraphQL schema to prevent test failures
// This catches schema mismatches early and ensures tests use realistic data

import { readFileSync, readdirSync, statSync } from "fs";
import { join } from "path";
import { describe, it, expect } from "vitest";

// Define the expected GraphQL schema types based on backend definitions
const SCHEMA_TYPES = {
  // User types - must match backend schema exactly
  User: {
    id: "string",
    username: "string",
    email: "string",
    role: "string",
    organization: "string",
    is_active: "boolean",
    email_verified: "boolean",
    created_at: "string",
    updated_at: "string",
    last_login_at: "string",
  },

  // System health types
  SystemHealth: {
    status: "string",
    metrics: "SystemMetrics",
    last_updated: "string",
  },

  SystemMetrics: {
    total_users: "number",
    active_users: "number",
    total_sessions: "number",
    active_sessions: "number",
    database_size_mb: "number",
    queue_items: "number",
    api_requests_per_minute: "number",
    average_response_time_ms: "number",
  },

  // Crawler types
  CrawlerConfig: {
    id: "string",
    name: "string",
    description: "string",
    is_enabled: "boolean",
    schedule: "string",
    data_sources: "DataSource[]",
    created_at: "string",
    updated_at: "string",
  },

  DataSource: {
    id: "string",
    name: "string",
    url: "string",
    type: "string",
    is_active: "boolean",
    last_checked: "string",
    status: "string",
  },

  CrawlerStatus: {
    is_running: "boolean",
    last_run: "string",
    next_run: "string",
    total_runs: "number",
    successful_runs: "number",
    failed_runs: "number",
  },

  QueueStatistics: {
    total_items: "number",
    pending_items: "number",
    processing_items: "number",
    completed_items: "number",
    failed_items: "number",
    average_processing_time: "number",
  },

  PerformanceMetrics: {
    cpu_usage: "number",
    memory_usage: "number",
    disk_usage: "number",
    network_latency: "number",
    response_time: "number",
    throughput: "number",
  },

  // Security types - must match backend schema exactly
  SecurityEvent: {
    id: "string",
    event_type: "string",
    user_id: "string",
    user_email: "string",
    ip_address: "string",
    user_agent: "string",
    description: "string",
    severity: "string",
    created_at: "string",
  },

  // Session types
  UserSession: {
    id: "string",
    user_id: "string",
    created_at: "string",
    last_activity: "string",
    expires_at: "string",
    user_agent: "string",
    ip_address: "string",
    is_active: "boolean",
  },
};

// Helper function to validate object structure against schema
function validateObject(obj: any, schema: any, path: string = ""): string[] {
  const errors: string[] = [];

  for (const [key, expectedType] of Object.entries(schema)) {
    const currentPath = path ? `${path}.${key}` : key;

    if (!(key in obj)) {
      errors.push(`Missing required field: ${currentPath}`);
      continue;
    }

    const value = obj[key];
    const actualType = Array.isArray(value) ? "array" : typeof value;

    if (expectedType === "array" && !Array.isArray(value)) {
      errors.push(`Expected array at ${currentPath}, got ${actualType}`);
    } else if (expectedType !== "array" && expectedType !== actualType) {
      errors.push(
        `Expected ${expectedType} at ${currentPath}, got ${actualType}`,
      );
    }

    // Recursively validate nested objects
    if (expectedType === "SystemMetrics" && typeof value === "object") {
      errors.push(
        ...validateObject(value, SCHEMA_TYPES.SystemMetrics, currentPath),
      );
    }
  }

  return errors;
}

// Helper function to get all JSON files in a directory
function getAllJsonFiles(dir: string): string[] {
  const files: string[] = [];

  function traverse(currentDir: string) {
    const items = readdirSync(currentDir);

    for (const item of items) {
      const fullPath = join(currentDir, item);
      const stat = statSync(fullPath);

      if (stat.isDirectory()) {
        traverse(fullPath);
      } else if (item.endsWith(".json")) {
        files.push(fullPath);
      }
    }
  }

  traverse(dir);
  return files;
}

describe("GraphQL Schema Validation", () => {
  const mockDir = join(process.cwd(), "src/__mocks__/graphql");

  it("validates all GraphQL mock files exist and are valid JSON", () => {
    const jsonFiles = getAllJsonFiles(mockDir);
    expect(jsonFiles.length).toBeGreaterThan(0);

    for (const filePath of jsonFiles) {
      expect(() => {
        const content = readFileSync(filePath, "utf-8");
        JSON.parse(content);
      }).not.toThrow(`Invalid JSON in ${filePath}`);
    }
  });

  it("validates getSystemHealth mock conforms to schema", () => {
    const systemHealthFiles = getAllJsonFiles(join(mockDir, "getSystemHealth"));

    for (const filePath of systemHealthFiles) {
      const content = readFileSync(filePath, "utf-8");
      const data = JSON.parse(content);

      // Validate the response structure
      expect(data).toHaveProperty("data");

      // Skip validation for error cases where data is null
      if (data.data === null) {
        return;
      }

      expect(data.data).toHaveProperty("systemHealth");

      // Validate systemHealth structure
      const systemHealth = data.data.systemHealth;
      const errors = validateObject(systemHealth, SCHEMA_TYPES.SystemHealth);

      if (errors.length > 0) {
        throw new Error(
          `Schema validation failed for ${filePath}:\n${errors.join("\n")}`,
        );
      }
    }
  });

  it("validates getUsers mock conforms to schema", () => {
    const userFiles = getAllJsonFiles(join(mockDir, "getUsers"));

    for (const filePath of userFiles) {
      const content = readFileSync(filePath, "utf-8");
      const data = JSON.parse(content);

      // Validate the response structure
      expect(data).toHaveProperty("data");

      // Skip validation for error cases where data is null
      if (data.data === null) {
        return;
      }

      expect(data.data).toHaveProperty("users");

      // Validate users array structure
      const users = data.data.users;
      expect(Array.isArray(users)).toBe(true);

      if (users.length > 0) {
        const errors = validateObject(users[0], SCHEMA_TYPES.User);
        if (errors.length > 0) {
          throw new Error(
            `Schema validation failed for ${filePath}:\n${errors.join("\n")}`,
          );
        }
      }
    }
  });

  it("validates getCrawlerConfig mock conforms to schema", () => {
    const configFiles = getAllJsonFiles(join(mockDir, "getCrawlerConfig"));

    for (const filePath of configFiles) {
      const content = readFileSync(filePath, "utf-8");
      const data = JSON.parse(content);

      // Validate the response structure
      expect(data).toHaveProperty("data");

      // Skip validation for error cases where data is null
      if (data.data === null) {
        return;
      }

      expect(data.data).toHaveProperty("crawlerConfig");

      // Validate crawlerConfig structure
      const config = data.data.crawlerConfig;
      const errors = validateObject(config, SCHEMA_TYPES.CrawlerConfig);

      if (errors.length > 0) {
        throw new Error(
          `Schema validation failed for ${filePath}:\n${errors.join("\n")}`,
        );
      }
    }
  });

  it("validates getDataSources mock conforms to schema", () => {
    const dataSourceFiles = getAllJsonFiles(join(mockDir, "getDataSources"));

    for (const filePath of dataSourceFiles) {
      const content = readFileSync(filePath, "utf-8");
      const data = JSON.parse(content);

      // Validate the response structure
      expect(data).toHaveProperty("data");
      // Skip validation for error cases where data is null
      if (data.data === null) {
        return;
      }

      expect(data.data).toHaveProperty("dataSources");

      // Validate dataSources array structure
      const dataSources = data.data.dataSources;
      expect(Array.isArray(dataSources)).toBe(true);

      if (dataSources.length > 0) {
        const errors = validateObject(dataSources[0], SCHEMA_TYPES.DataSource);
        if (errors.length > 0) {
          throw new Error(
            `Schema validation failed for ${filePath}:\n${errors.join("\n")}`,
          );
        }
      }
    }
  });

  it("validates getSecurityEvents mock conforms to schema", () => {
    const securityFiles = getAllJsonFiles(join(mockDir, "getSecurityEvents"));

    for (const filePath of securityFiles) {
      const content = readFileSync(filePath, "utf-8");
      const data = JSON.parse(content);

      // Validate the response structure
      expect(data).toHaveProperty("data");
      expect(data.data).toHaveProperty("securityEvents");

      // Validate securityEvents array structure
      const events = data.data.securityEvents;
      expect(Array.isArray(events)).toBe(true);

      if (events.length > 0) {
        const errors = validateObject(events[0], SCHEMA_TYPES.SecurityEvent);
        if (errors.length > 0) {
          throw new Error(
            `Schema validation failed for ${filePath}:\n${errors.join("\n")}`,
          );
        }
      }
    }
  });

  it("validates getOnlineUsers mock conforms to schema", () => {
    const onlineUserFiles = getAllJsonFiles(join(mockDir, "getOnlineUsers"));

    for (const filePath of onlineUserFiles) {
      const content = readFileSync(filePath, "utf-8");
      const data = JSON.parse(content);

      // Validate the response structure
      expect(data).toHaveProperty("data");
      // Skip validation for error cases where data is null
      if (data.data === null) {
        return;
      }

      expect(data.data).toHaveProperty("onlineUsers");

      // Validate onlineUsers array structure
      const users = data.data.onlineUsers;
      expect(Array.isArray(users)).toBe(true);

      if (users.length > 0) {
        const errors = validateObject(users[0], SCHEMA_TYPES.User);
        if (errors.length > 0) {
          throw new Error(
            `Schema validation failed for ${filePath}:\n${errors.join("\n")}`,
          );
        }
      }
    }
  });

  it("validates getPerformanceMetrics mock conforms to schema", () => {
    const metricsFiles = getAllJsonFiles(
      join(mockDir, "getPerformanceMetrics"),
    );

    for (const filePath of metricsFiles) {
      const content = readFileSync(filePath, "utf-8");
      const data = JSON.parse(content);

      // Validate the response structure
      expect(data).toHaveProperty("data");
      // Skip validation for error cases where data is null
      if (data.data === null) {
        return;
      }

      expect(data.data).toHaveProperty("performanceMetrics");

      // Validate performanceMetrics structure
      const metrics = data.data.performanceMetrics;
      const errors = validateObject(metrics, SCHEMA_TYPES.PerformanceMetrics);

      if (errors.length > 0) {
        throw new Error(
          `Schema validation failed for ${filePath}:\n${errors.join("\n")}`,
        );
      }
    }
  });

  it("validates getQueueStatistics mock conforms to schema", () => {
    const queueFiles = getAllJsonFiles(join(mockDir, "getQueueStatistics"));

    for (const filePath of queueFiles) {
      const content = readFileSync(filePath, "utf-8");
      const data = JSON.parse(content);

      // Validate the response structure
      expect(data).toHaveProperty("data");
      // Skip validation for error cases where data is null
      if (data.data === null) {
        return;
      }

      expect(data.data).toHaveProperty("queueStatistics");

      // Validate queueStatistics structure
      const stats = data.data.queueStatistics;
      const errors = validateObject(stats, SCHEMA_TYPES.QueueStatistics);

      if (errors.length > 0) {
        throw new Error(
          `Schema validation failed for ${filePath}:\n${errors.join("\n")}`,
        );
      }
    }
  });

  it("validates getCrawlerStatus mock conforms to schema", () => {
    const statusFiles = getAllJsonFiles(join(mockDir, "getCrawlerStatus"));

    for (const filePath of statusFiles) {
      const content = readFileSync(filePath, "utf-8");
      const data = JSON.parse(content);

      // Validate the response structure
      expect(data).toHaveProperty("data");
      // Skip validation for error cases where data is null
      if (data.data === null) {
        return;
      }

      expect(data.data).toHaveProperty("crawlerStatus");

      // Validate crawlerStatus structure
      const status = data.data.crawlerStatus;
      const errors = validateObject(status, SCHEMA_TYPES.CrawlerStatus);

      if (errors.length > 0) {
        throw new Error(
          `Schema validation failed for ${filePath}:\n${errors.join("\n")}`,
        );
      }
    }
  });

  it("validates getCurrentUser mock conforms to schema", () => {
    const userFiles = getAllJsonFiles(join(mockDir, "getCurrentUser"));

    for (const filePath of userFiles) {
      const content = readFileSync(filePath, "utf-8");
      const data = JSON.parse(content);

      // Validate the response structure
      expect(data).toHaveProperty("data");
      expect(data.data).toHaveProperty("currentUser");

      // Validate currentUser structure
      const user = data.data.currentUser;
      const errors = validateObject(user, SCHEMA_TYPES.User);

      if (errors.length > 0) {
        throw new Error(
          `Schema validation failed for ${filePath}:\n${errors.join("\n")}`,
        );
      }
    }
  });
});
