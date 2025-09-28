/**
 * MSW handlers for Jest tests
 * Based on MSW examples: https://github.com/mswjs/examples/blob/main/examples/with-jest/
 */

import { http } from "msw";

// Debug flag - set to true to enable MSW debug output
const MSW_DEBUG = process.env.MSW_DEBUG === "true" || false;

export const handlers = [
  // Monitoring API endpoints
  http.get("/api/monitoring/system-status", () => {
    if (MSW_DEBUG) console.log("[MSW] System status request intercepted");
    const response = {
      overall: "healthy",
      services: {
        backend: "healthy",
        database: "healthy",
        crawler: "warning",
        grafana: "healthy",
      },
      alerts: 2,
    };
    if (MSW_DEBUG) console.log("[MSW] Returning system status:", response);
    return new Response(JSON.stringify(response), {
      headers: {
        "Content-Type": "application/json",
      },
    });
  }),

  // Admin authentication endpoints
  http.get("/api/admin/auth/validate", () => {
    return new Response(
      JSON.stringify({
        id: "test-user",
        username: "admin",
        email: "admin@example.com",
        role: "super_admin",
        permissions: ["view_dashboard", "manage_users", "system_config"],
        lastLogin: "2024-01-15T10:30:00Z",
        sessionExpiry: new Date(Date.now() + 3600000).toISOString(),
      }),
      {
        headers: {
          "Content-Type": "application/json",
        },
      },
    );
  }),

  http.post("/api/admin/auth/login", () => {
    return new Response(
      JSON.stringify({
        user: {
          id: "test-user",
          username: "admin",
          email: "admin@example.com",
          role: "super_admin",
          permissions: ["view_dashboard", "manage_users", "system_config"],
          lastLogin: "2024-01-15T10:30:00Z",
          sessionExpiry: new Date(Date.now() + 3600000).toISOString(),
        },
        token: "mock-jwt-token",
      }),
      {
        headers: {
          "Content-Type": "application/json",
        },
      },
    );
  }),

  http.post("/api/admin/auth/refresh", () => {
    return new Response(
      JSON.stringify({
        user: {
          id: "test-user",
          username: "admin",
          email: "admin@example.com",
          role: "super_admin",
          permissions: ["view_dashboard", "manage_users", "system_config"],
          lastLogin: "2024-01-15T10:30:00Z",
          sessionExpiry: new Date(Date.now() + 3600000).toISOString(),
        },
        token: "mock-refreshed-jwt-token",
      }),
      {
        headers: {
          "Content-Type": "application/json",
        },
      },
    );
  }),

  http.post("/api/admin/auth/logout", () => {
    return new Response(null, { status: 200 });
  }),

  http.post("/api/admin/security/events", () => {
    return new Response(null, { status: 200 });
  }),

  // GraphQL endpoints
  http.post("/graphql", async ({ request }) => {
    if (MSW_DEBUG) console.log("[MSW] GraphQL request intercepted");

    const body = await request.json();
    const { query, variables } = body;

    if (MSW_DEBUG) {
      console.log("[MSW] Query:", query);
      console.log("[MSW] Variables:", variables);
      console.log("[MSW] Checking query:", query.includes("GetCrawlerStatus"));
    }

    if (query.includes("GetCrawlerStatus")) {
      if (MSW_DEBUG) console.log("[MSW] Returning crawler status");

      // Add a small delay to let React Query "latch" and be ready to receive data
      await new Promise((resolve) => setTimeout(resolve, 50));

      return new Response(
        JSON.stringify({
          data: {
            crawlerStatus: {
              is_running: true,
              is_paused: false,
              current_task: "Processing FRED data",
              progress: 0.65,
              last_update: "2024-01-15T10:30:00Z",
              error: null,
              active_workers: 3,
              last_crawl: "2024-01-15T10:30:00Z",
              next_scheduled_crawl: "2024-01-15T11:30:00Z",
            },
          },
        }),
        {
          headers: {
            "Content-Type": "application/json",
          },
        },
      );
    }

    if (query.includes("GetQueueStatistics")) {
      if (MSW_DEBUG) console.log("[MSW] Returning queue statistics");

      // Add a small delay to let React Query "latch" and be ready to receive data
      await new Promise((resolve) => setTimeout(resolve, 50));

      return new Response(
        JSON.stringify({
          data: {
            queueStatistics: {
              total_items: 1247,
              processed_items: 1200,
              failed_items: 21,
              pending_items: 23,
              processing_items: 3,
              completed_items: 1200,
              retrying_items: 0,
              oldest_pending: "2024-01-15T10:30:00Z",
              average_processing_time: 2.3,
            },
          },
        }),
        {
          headers: {
            "Content-Type": "application/json",
          },
        },
      );
    }

    if (query.includes("GetPerformanceMetrics")) {
      if (MSW_DEBUG) console.log("[MSW] Returning performance metrics");

      // Add a small delay to let React Query "latch" and be ready to receive data
      await new Promise((resolve) => setTimeout(resolve, 50));

      return new Response(
        JSON.stringify({
          data: {
            performanceMetrics: [
              {
                timestamp: "2024-01-15T10:30:00Z",
                cpu_usage_percent: 45.2,
                memory_usage_percent: 2.1,
                queue_depth: 23,
                error_rate_percent: 1.7,
                throughput_per_minute: 7.5,
                average_response_time_ms: 2300,
              },
            ],
          },
        }),
        {
          headers: {
            "Content-Type": "application/json",
          },
        },
      );
    }

    if (query.includes("GetCrawlerLogs")) {
      if (MSW_DEBUG) console.log("[MSW] Returning crawler logs");

      // Add a small delay to let React Query "latch" and be ready to receive data
      await new Promise((resolve) => setTimeout(resolve, 50));

      return new Response(
        JSON.stringify({
          data: {
            crawlerLogs: [
              {
                id: "log1",
                timestamp: "2024-01-15T10:00:00Z",
                level: "info",
                source: "FRED",
                message: "Successfully crawled GDP data",
                details: null,
                duration_ms: 2300,
                status: "success",
                created_at: "2024-01-15T10:00:00Z",
              },
              {
                id: "log2",
                timestamp: "2024-01-15T10:29:45Z",
                level: "warning",
                source: "crawler",
                message: "Rate limit approaching for API endpoint",
                details: null,
                duration_ms: null,
                status: null,
                created_at: "2024-01-15T10:29:45Z",
              },
            ],
          },
        }),
        {
          headers: {
            "Content-Type": "application/json",
          },
        },
      );
    }

    if (query.includes("GetSystemHealth")) {
      return new Response(
        JSON.stringify({
          data: {
            systemHealth: {
              overall_status: "HEALTHY",
              components: [
                {
                  name: "Database",
                  status: "RUNNING",
                  last_check: "2024-01-15T10:30:00Z",
                  message: "All connections healthy",
                },
                {
                  name: "API Server",
                  status: "RUNNING",
                  last_check: "2024-01-15T10:30:00Z",
                  message: "All endpoints responding",
                },
                {
                  name: "Cache",
                  status: "DEGRADED",
                  last_check: "2024-01-15T10:30:00Z",
                  message: "High memory usage",
                },
              ],
              uptime_seconds: 86400,
              version: "1.0.0",
              environment: "production",
            },
          },
        }),
        {
          headers: {
            "Content-Type": "application/json",
          },
        },
      );
    }

    if (query.includes("GetUsers")) {
      const mockUsers = [
        {
          id: "1",
          name: "John Administrator",
          email: "john.admin@company.com",
          role: "super_admin",
          status: "active",
          lastLogin: "2024-01-15T10:30:00Z",
          createdAt: "2023-06-01T00:00:00Z",
          isOnline: true,
          sessionId: "sess_abc123",
          ipAddress: "192.168.1.100",
          userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)",
        },
        {
          id: "2",
          name: "Jane Manager",
          email: "jane.manager@company.com",
          role: "admin",
          status: "active",
          lastLogin: "2024-01-15T09:15:00Z",
          createdAt: "2023-08-15T00:00:00Z",
          isOnline: true,
          sessionId: "sess_def456",
          ipAddress: "192.168.1.101",
          userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
        },
        {
          id: "3",
          name: "Bob Analyst",
          email: "bob.analyst@company.com",
          role: "read_only",
          status: "active",
          lastLogin: "2024-01-14T16:45:00Z",
          createdAt: "2023-09-20T00:00:00Z",
          isOnline: false,
          sessionId: "sess_ghi789",
          ipAddress: "192.168.1.102",
          userAgent: "Mozilla/5.0 (X11; Linux x86_64)",
        },
        {
          id: "4",
          name: "Alice Developer",
          email: "alice.dev@company.com",
          role: "admin",
          status: "inactive",
          lastLogin: "2024-01-10T14:20:00Z",
          createdAt: "2023-07-10T00:00:00Z",
          isOnline: false,
        },
      ];

      // Apply search filtering if provided
      let filteredUsers = mockUsers;
      if (variables?.search) {
        filteredUsers = mockUsers.filter(
          (user) =>
            user.name.toLowerCase().includes(variables.search.toLowerCase()) ||
            user.email.toLowerCase().includes(variables.search.toLowerCase()),
        );
      }

      // Apply role filtering if provided
      if (variables?.role && variables.role !== "all") {
        filteredUsers = filteredUsers.filter(
          (user) => user.role === variables.role,
        );
      }

      return new Response(
        JSON.stringify({
          data: {
            users: filteredUsers,
          },
        }),
        {
          status: 200,
          headers: {
            "Content-Type": "application/json",
          },
        },
      );
    }

    if (query.includes("GetOnlineUsers")) {
      return new Response(
        JSON.stringify({
          data: {
            onlineUsers: [
              {
                id: "1",
                name: "John Administrator",
                email: "john.admin@company.com",
                role: "super_admin",
                status: "active",
                lastLogin: "2024-01-15T10:30:00Z",
                createdAt: "2023-06-01T00:00:00Z",
                isOnline: true,
                sessionId: "sess_abc123",
                ipAddress: "192.168.1.100",
                userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)",
              },
              {
                id: "2",
                name: "Jane Manager",
                email: "jane.manager@company.com",
                role: "admin",
                status: "active",
                lastLogin: "2024-01-15T09:15:00Z",
                createdAt: "2023-08-15T00:00:00Z",
                isOnline: true,
                sessionId: "sess_def456",
                ipAddress: "192.168.1.101",
                userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64)",
              },
            ],
          },
        }),
        {
          status: 200,
          headers: {
            "Content-Type": "application/json",
          },
        },
      );
    }

    // Default response for other GraphQL operations
    return new Response(
      JSON.stringify({
        data: {
          crawlerLogs: [
            {
              id: "log1",
              timestamp: "2024-01-15T10:00:00Z",
              level: "info",
              source: "FRED",
              message: "Successfully crawled GDP data",
              duration: 2.3,
              status: "success",
            },
          ],
        },
      }),
      {
        status: 200,
        headers: {
          "Content-Type": "application/json",
        },
      },
    );
  }),
];
