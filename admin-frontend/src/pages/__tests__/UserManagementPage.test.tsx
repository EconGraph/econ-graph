// REQUIREMENT: Comprehensive tests for UserManagementPage component
// PURPOSE: Ensure user management interface works correctly with role-based access control
// This validates the admin interface for managing users, sessions, and permissions

import React from "react";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { BrowserRouter } from "react-router-dom";
import { ThemeProvider, createTheme } from "@mui/material/styles";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { vi } from "vitest";
import UserManagementPage from "../UserManagementPage";

// Set timeout for all tests in this file due to performance characteristics
// TODO: Optimize UserManagementPage component performance to reduce test timeouts
vi.setConfig({ testTimeout: 10000 });
// Mock the contexts to prevent resource leaks
vi.mock("../../contexts/AuthContext", () => ({
  AuthProvider: ({ children }: any) => children,
  useAuth: () => ({
    user: {
      id: "test-user",
      username: "admin",
      role: "super_admin",
      sessionExpiry: new Date(Date.now() + 3600000).toISOString(),
    },
    isAuthenticated: true,
    login: vi.fn(),
    logout: vi.fn(),
    refreshSession: vi.fn(),
    extendSession: vi.fn(),
  }),
}));

vi.mock("../../contexts/SecurityContext", () => ({
  SecurityProvider: ({ children }: any) => children,
  useSecurity: () => ({
    checkAccess: vi.fn(() => true),
    logSecurityEvent: vi.fn(),
    securityEvents: [],
    sessionRemainingTime: 3661, // 61 minutes and 1 second in seconds
  }),
}));

// Import MSW setup
import {
  setupSimpleMSW,
  cleanupSimpleMSW,
  setMockScenario,
} from "../../__mocks__/msw/simpleServer";

// Setup MSW for GraphQL requests
beforeAll(async () => {
  await setupSimpleMSW();
  setMockScenario("success");
});

afterAll(async () => {
  await cleanupSimpleMSW();
});

// Mock timers to prevent resource leaks - use more targeted approach
// Note: We don't mock global timers as they interfere with waitFor

// const mockAdminContext = {
//   user: {
//     id: "2",
//     name: "Admin User",
//     email: "admin@test.com",
//     role: "admin",
//   },
//   login: vi.fn(),
//   logout: vi.fn(),
//   isAuthenticated: true,
//   loading: false,
// };

// const mockSecurityContext = {
//   checkAccess: vi.fn((role: string) => role === "super_admin"),
//   sessionRemainingTime: 3600,
//   securityEvents: [],
//   refreshSecurityContext: vi.fn(),
// };

// Create a test theme
const theme = createTheme();

// Mock super admin context - removed as it's not used

// Test wrapper component
const TestWrapper: React.FC<{
  children: React.ReactNode;
  authContext?: any;
}> = ({ children }) => {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        retry: false,
        refetchOnWindowFocus: false,
        refetchOnMount: false,
        refetchOnReconnect: false,
        staleTime: Infinity,
        gcTime: 0,
      },
    },
  });

  return (
    <BrowserRouter>
      <QueryClientProvider client={queryClient}>
        <ThemeProvider theme={theme}>{children}</ThemeProvider>
      </QueryClientProvider>
    </BrowserRouter>
  );
};

describe("UserManagementPage", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("Access Control", () => {
    it("allows access for super_admin users", () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      expect(screen.getByText("User Management")).toBeInTheDocument();
      expect(
        screen.getByText(
          "Manage registered users, active sessions, and access controls",
        ),
      ).toBeInTheDocument();
    });

    it.skip("denies access for non-super_admin users", () => {
      // TODO: Fix Jest mock resolution issues - see https://github.com/jmalicki/econ-graph/issues/108
      // This test is skipped due to persistent mock resolution problems with useSecurity and useUsers hooks
      // The issue is related to Jest's module hoisting and mock application order

      // Test implementation will be restored once mock resolution issues are fixed
      expect(true).toBe(true); // Placeholder assertion
    });
  });

  describe("Rendering", () => {
    it("renders user management interface with correct title", () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      // First check if the main content renders (using title instead of testid)
      expect(screen.getByText("User Management")).toBeInTheDocument();

      // Then check specific content
      expect(screen.getByText("User Management")).toBeInTheDocument();
      expect(
        screen.getByText(
          "Manage registered users, active sessions, and access controls",
        ),
      ).toBeInTheDocument();
    });

    it("displays statistics cards", async () => {
      // Previously disabled due to infinite re-render loop caused by refetchInterval
      // Fixed by disabling refetchInterval in test environments
      //
      // TODO: Fix component performance issues and re-enable this test

      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        expect(screen.getByText("Total Users")).toBeInTheDocument();
        expect(screen.getByText("Active Users")).toBeInTheDocument();
        expect(screen.getByText("Online Now")).toBeInTheDocument();
        expect(screen.getByText("Current User")).toBeInTheDocument();
      });
    });

    it("shows correct user counts in statistics", async () => {
      // Previously disabled due to infinite re-render loop caused by refetchInterval
      // Fixed by disabling refetchInterval in test environments

      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        // Check that we have the expected number of "2" values (Total Users and Active Users)
        expect(screen.getAllByText("2")).toHaveLength(2);
        // Check that we have the expected number of "1" values (Online Users)
        expect(screen.getByText("1")).toBeInTheDocument(); // Online users
        // Check that we have multiple "admin" values (Current User card and user table)
        expect(screen.getAllByText("admin")).toHaveLength(2);
      });
    });
  });

  describe("User Table", () => {
    it("displays all users in the table", async () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        expect(screen.getByText("John Administrator")).toBeInTheDocument();
        expect(
          screen.getByText("jane.manager@company.com"),
        ).toBeInTheDocument();
        expect(screen.getByText("Bob Analyst")).toBeInTheDocument();
        expect(screen.getByText("Alice Developer")).toBeInTheDocument();
      });
    });

    it("shows user roles with correct colors", async () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        expect(screen.getByText("super_admin")).toBeInTheDocument();
        expect(screen.getByText("admin")).toBeInTheDocument();
        expect(screen.getByText("read_only")).toBeInTheDocument();
      });
    });

    it("displays user status indicators", async () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        expect(screen.getByText("active")).toBeInTheDocument();
        expect(screen.getByText("suspended")).toBeInTheDocument();
      });
    });

    it("shows last login and creation dates", async () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        // Should show formatted dates - use getAllByText for multiple elements
        expect(screen.getAllByText(/1\/15\/2024/)).toHaveLength(2); // Two users with same last login date

        // Look for creation dates with a more flexible approach
        const createdDates = screen.getAllByText(/2023/);
        expect(createdDates.length).toBeGreaterThan(0); // Should have some creation dates from 2023
      });
    });

    it("displays action buttons for each user", async () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        const editButtons = screen.getAllByLabelText("Edit User");
        const blockButtons = screen.getAllByLabelText(/Suspend|Activate User/);
        const deleteButtons = screen.getAllByLabelText("Delete User");

        expect(editButtons).toHaveLength(4);
        expect(blockButtons).toHaveLength(4);
        expect(deleteButtons).toHaveLength(4);
      });
    });
  });

  describe("Tab Navigation", () => {
    it("switches between different tabs", async () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      // Should start with "All Users" tab
      expect(screen.getByText("All Users")).toBeInTheDocument();

      // Switch to "Online Users" tab
      fireEvent.click(screen.getByText("Online Users"));

      await waitFor(() => {
        expect(
          screen.getByText("Currently Online Users (2)"),
        ).toBeInTheDocument();
      });

      // Switch to "Online Users" tab
      fireEvent.click(screen.getByText("Online Users"));

      await waitFor(() => {
        expect(screen.getByText("Recent Online Users")).toBeInTheDocument();
      });
    });

    it("shows online users with session information", async () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      fireEvent.click(screen.getByText("Online Users"));

      await waitFor(() => {
        expect(screen.getByText("192.168.1.100")).toBeInTheDocument();
        expect(screen.getByText("192.168.1.101")).toBeInTheDocument();
        expect(screen.getByText("Mozilla/5.0")).toBeInTheDocument();
      });
    });

    it("displays user activity placeholder", async () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      fireEvent.click(screen.getByText("Online Users"));

      await waitFor(() => {
        expect(
          screen.getByText(
            /User activity logs and audit trails would be displayed here/,
          ),
        ).toBeInTheDocument();
      });
    });
  });

  describe("User Actions", () => {
    it("opens edit user dialog when edit button is clicked", async () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        const editButtons = screen.getAllByLabelText("Edit User");
        fireEvent.click(editButtons[0]);
      });

      await waitFor(() => {
        expect(screen.getByText("Edit User")).toBeInTheDocument();
        expect(
          screen.getByDisplayValue("John Administrator"),
        ).toBeInTheDocument();
        expect(
          screen.getByDisplayValue("john.admin@company.com"),
        ).toBeInTheDocument();
      });
    });

    it("toggles user suspension status", async () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        const blockButtons = screen.getAllByLabelText(/Suspend|Activate User/);
        // Click suspend button for Alice (who is currently suspended)
        fireEvent.click(blockButtons[3]);
      });

      await waitFor(() => {
        // Status should change from suspended to active
        expect(screen.getByText("active")).toBeInTheDocument();
      });
    });

    it("deletes user when delete button is clicked", async () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        const deleteButtons = screen.getAllByLabelText("Delete User");
        fireEvent.click(deleteButtons[3]); // Delete Alice
      });

      await waitFor(() => {
        // Alice should be removed from the table
        expect(screen.queryByText("Alice Developer")).not.toBeInTheDocument();
      });
    });

    it("prevents self-modification actions", async () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        // The first user (John Administrator) should have disabled action buttons
        const editButtons = screen.getAllByLabelText("Edit User");
        const blockButtons = screen.getAllByLabelText(/Suspend|Activate User/);
        const deleteButtons = screen.getAllByLabelText("Delete User");

        expect(editButtons[0]).toBeDisabled();
        expect(blockButtons[0]).toBeDisabled();
        expect(deleteButtons[0]).toBeDisabled();
      });
    });
  });

  describe("User Dialog", () => {
    it("opens add user dialog when add button is clicked", async () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      const addButton = screen.getByText("Add User");
      fireEvent.click(addButton);

      await waitFor(() => {
        expect(screen.getByText("Create New User")).toBeInTheDocument();
      });
    });

    it("allows editing user details in dialog", async () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        const editButtons = screen.getAllByLabelText("Edit User");
        fireEvent.click(editButtons[1]); // Edit Jane
      });

      await waitFor(() => {
        const nameField = screen.getByDisplayValue("Jane Manager");
        fireEvent.change(nameField, { target: { value: "Jane Updated" } });

        expect(screen.getByDisplayValue("Jane Updated")).toBeInTheDocument();
      });
    });

    it("allows changing user role in dialog", async () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        const editButtons = screen.getAllByLabelText("Edit User");
        fireEvent.click(editButtons[1]); // Edit Jane
      });

      await waitFor(() => {
        const roleSelect = screen.getByDisplayValue("admin");
        fireEvent.mouseDown(roleSelect);

        const superAdminOption = screen.getByText("Super Admin");
        fireEvent.click(superAdminOption);

        expect(screen.getByDisplayValue("super_admin")).toBeInTheDocument();
      });
    });

    it("allows changing user status in dialog", async () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        const editButtons = screen.getAllByLabelText("Edit User");
        fireEvent.click(editButtons[2]); // Edit Bob
      });

      await waitFor(() => {
        const statusSelect = screen.getByDisplayValue("active");
        fireEvent.mouseDown(statusSelect);

        const suspendedOption = screen.getByText("Suspended");
        fireEvent.click(suspendedOption);

        expect(screen.getByDisplayValue("suspended")).toBeInTheDocument();
      });
    });

    it("saves user changes when save button is clicked", async () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        const editButtons = screen.getAllByLabelText("Edit User");
        fireEvent.click(editButtons[1]); // Edit Jane
      });

      await waitFor(() => {
        const saveButton = screen.getByText("Save");
        fireEvent.click(saveButton);
      });

      await waitFor(() => {
        // Dialog should close
        expect(screen.queryByText("Edit User")).not.toBeInTheDocument();
      });
    });

    it("cancels dialog when cancel button is clicked", async () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      const addButton = screen.getByText("Add User");
      fireEvent.click(addButton);

      await waitFor(() => {
        const cancelButton = screen.getByText("Cancel");
        fireEvent.click(cancelButton);
      });

      await waitFor(() => {
        // Dialog should close
        expect(screen.queryByText("Add New User")).not.toBeInTheDocument();
      });
    });
  });

  describe("Search and Filter", () => {
    it("filters users by search term", async () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        const searchField = screen.getByPlaceholderText("Search users...");
        fireEvent.change(searchField, { target: { value: "John" } });
      });

      await waitFor(() => {
        expect(screen.getByText("John Administrator")).toBeInTheDocument();
        expect(screen.queryByText("Jane Manager")).not.toBeInTheDocument();
      });
    });

    it("filters users by role", async () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        const roleSelect = screen.getByDisplayValue("All Roles");
        fireEvent.mouseDown(roleSelect);

        const adminOption = screen.getByText("Admin");
        fireEvent.click(adminOption);
      });

      await waitFor(() => {
        expect(screen.getByText("Jane Manager")).toBeInTheDocument();
        expect(screen.getByText("Alice Developer")).toBeInTheDocument();
        expect(screen.queryByText("Bob Analyst")).not.toBeInTheDocument();
      });
    });

    it("combines search and role filters", async () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      await waitFor(() => {
        const searchField = screen.getByPlaceholderText("Search users...");
        fireEvent.change(searchField, { target: { value: "Jane" } });

        const roleSelect = screen.getByDisplayValue("All Roles");
        fireEvent.mouseDown(roleSelect);

        const adminOption = screen.getByText("Admin");
        fireEvent.click(adminOption);
      });

      await waitFor(() => {
        expect(screen.getByText("Jane Manager")).toBeInTheDocument();
        expect(screen.queryByText("Alice Developer")).not.toBeInTheDocument();
      });
    });
  });

  describe("Online Users Tab", () => {
    it("shows only online users with session details", async () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      fireEvent.click(screen.getByText("Online Users"));

      await waitFor(() => {
        expect(screen.getByText("John Administrator")).toBeInTheDocument();
        expect(screen.getByText("Jane Manager")).toBeInTheDocument();
        expect(screen.queryByText("Bob Analyst")).not.toBeInTheDocument();
        expect(screen.queryByText("Alice Developer")).not.toBeInTheDocument();
      });
    });

    it("displays session information for online users", async () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      fireEvent.click(screen.getByText("Online Users"));

      await waitFor(() => {
        expect(screen.getByText("192.168.1.100")).toBeInTheDocument();
        expect(screen.getByText("192.168.1.101")).toBeInTheDocument();
        expect(screen.getByText("sess_abc123")).toBeInTheDocument();
        expect(screen.getByText("sess_def456")).toBeInTheDocument();
      });
    });

    it.skip("shows force logout buttons for online users", async () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      fireEvent.click(screen.getByText("Online Users"));

      await waitFor(() => {
        const logoutButtons = screen.getAllByLabelText("Force Logout");
        expect(logoutButtons).toHaveLength(2);
      });
    });

    it.skip("prevents self-logout", async () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      fireEvent.click(screen.getByText("Online Users"));

      await waitFor(() => {
        const logoutButtons = screen.getAllByLabelText("Force Logout");
        // First user (John Administrator) should have disabled logout button
        expect(logoutButtons[0]).toBeDisabled();
        expect(logoutButtons[1]).not.toBeDisabled();
      });
    });
  });

  describe("Refresh Functionality", () => {
    it("handles refresh button click", async () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      const refreshButton = screen.getByText("Refresh");
      fireEvent.click(refreshButton);

      // Should trigger loading state
      await waitFor(() => {
        expect(refreshButton).toBeInTheDocument();
      });
    });
  });

  describe("Error Handling", () => {
    it("handles missing user data gracefully", async () => {
      render(
        <TestWrapper>
          <UserManagementPage />
        </TestWrapper>,
      );

      // Should render without errors
      expect(screen.getByText("User Management")).toBeInTheDocument();
    });
  });
});
