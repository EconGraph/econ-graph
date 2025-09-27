/**
 * MSW handlers for Jest tests
 * Based on MSW examples: https://github.com/mswjs/examples/blob/main/examples/with-jest/
 */

import { http } from "msw";

export const handlers = [
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
    console.log("[MSW] GraphQL request intercepted");

    const body = await request.json();
    const { query, variables } = body;

    // Handle different GraphQL operations
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
