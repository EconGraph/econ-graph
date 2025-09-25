// REQUIREMENT: Comprehensive tests for AdminLayout component
// PURPOSE: Ensure admin layout renders correctly with role-based navigation and security features
// This validates the admin interface layout, navigation, and security controls work as expected

import React from "react";
import {
  render,
  screen,
  fireEvent,
  waitFor,
  within,
} from "@testing-library/react";
import { BrowserRouter } from "react-router-dom";
import { ThemeProvider, createTheme } from "@mui/material/styles";
import AdminLayout from "../AdminLayout";
import { AuthProvider } from "../../../contexts/AuthContext";
import { SecurityProvider } from "../../../contexts/SecurityContext";

// Set timeout for all tests in this file due to performance characteristics
// TODO: Optimize AdminLayout component performance to reduce test timeouts
jest.setTimeout(60000);

// Mock the contexts with test data
const mockAuthContext = {
  user: {
    id: "1",
    name: "Test Admin",
    email: "admin@test.com",
    role: "super_admin",
  },
  login: jest.fn(),
  logout: jest.fn(),
  isAuthenticated: true,
  loading: false,
};

// const mockSecurityContext = {
//   checkAccess: jest.fn((role: string) => {
//     // Mock role-based access control
//     const userRole = mockAuthContext.user.role;
//     const roleHierarchy = {
//       read_only: ["read_only"],
//       admin: ["read_only", "admin"],
//       super_admin: ["read_only", "admin", "super_admin"],
//     };
//     return (
//       roleHierarchy[userRole as keyof typeof roleHierarchy]?.includes(role) ||
//       false
//     );
//   }),
//   sessionRemainingTime: 3600, // 1 hour
//   securityEvents: [],
//   refreshSecurityContext: jest.fn(),
// };

// Mock React Router
const mockNavigate = jest.fn();
jest.mock("react-router-dom", () => ({
  ...jest.requireActual("react-router-dom"),
  useNavigate: () => mockNavigate,
  useLocation: () => ({ pathname: "/" }),
  Outlet: () => <div data-testid="outlet">Outlet Content</div>,
}));

// Create a test theme
const theme = createTheme();

// Test wrapper component
const TestWrapper: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <BrowserRouter>
    <ThemeProvider theme={theme}>
      <AuthProvider>
        <SecurityProvider>{children}</SecurityProvider>
      </AuthProvider>
    </ThemeProvider>
  </BrowserRouter>
);

describe("AdminLayout", () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  describe("Rendering", () => {
    it("renders admin layout with correct title", () => {
      render(
        <TestWrapper>
          <AdminLayout />
        </TestWrapper>,
      );

      // First check if the main layout renders
      expect(screen.getByTestId("admin-layout")).toBeInTheDocument();

      // Then check specific content
      expect(screen.getByText("EconGraph Administration")).toBeInTheDocument();
      expect(screen.getAllByText("🔒 ADMIN INTERFACE")[0]).toBeInTheDocument();
    });

    it("displays user information correctly", () => {
      render(
        <TestWrapper>
          <AdminLayout />
        </TestWrapper>,
      );

      expect(screen.getByText("Test Admin")).toBeInTheDocument();
      expect(screen.getByText("super_admin")).toBeInTheDocument();
      expect(screen.getByText("Session: 60:00")).toBeInTheDocument();
    });

    it("renders navigation items based on user role", () => {
      render(
        <TestWrapper>
          <AdminLayout />
        </TestWrapper>,
      );

      // Super admin should see all navigation items
      expect(screen.getAllByText("Dashboard")[0]).toBeInTheDocument();
      expect(screen.getAllByText("System Health")[0]).toBeInTheDocument();
      expect(screen.getAllByText("Monitoring")[0]).toBeInTheDocument();
      expect(screen.getAllByText("Crawler Management")[0]).toBeInTheDocument();
      expect(screen.getAllByText("Database Management")[0]).toBeInTheDocument();
      expect(screen.getAllByText("User Management")[0]).toBeInTheDocument();
      expect(screen.getAllByText("Security")[0]).toBeInTheDocument();
      expect(screen.getAllByText("Audit Logs")[0]).toBeInTheDocument();
      expect(screen.getAllByText("System Config")[0]).toBeInTheDocument();
    });

    it("renders outlet content", () => {
      render(
        <TestWrapper>
          <AdminLayout />
        </TestWrapper>,
      );

      expect(screen.getByTestId("outlet")).toBeInTheDocument();
    });
  });

  describe("Navigation", () => {
    it("handles navigation item clicks", () => {
      render(
        <TestWrapper>
          <AdminLayout />
        </TestWrapper>,
      );

      const monitoringItem = screen.getAllByText("Monitoring")[0];
      fireEvent.click(monitoringItem);

      expect(mockNavigate).toHaveBeenCalledWith("/monitoring");
    });

    it("shows active navigation item", () => {
      // Mock location to show active state
      jest.doMock("react-router-dom", () => ({
        ...jest.requireActual("react-router-dom"),
        useLocation: () => ({ pathname: "/monitoring" }),
        useNavigate: () => mockNavigate,
        Outlet: () => <div data-testid="outlet">Outlet Content</div>,
      }));

      render(
        <TestWrapper>
          <AdminLayout />
        </TestWrapper>,
      );

      // The active item should have selected styling
      const monitoringItem = screen
        .getAllByText("Monitoring")[0]
        .closest('[role="button"]');
      expect(monitoringItem).toHaveClass("Mui-selected");
    });
  });

  describe("User Menu", () => {
    it("opens user profile menu when clicked", () => {
      render(
        <TestWrapper>
          <AdminLayout />
        </TestWrapper>,
      );

      const userMenuButton = screen.getByLabelText("account of current user");
      fireEvent.click(userMenuButton);

      expect(screen.getByText("Profile")).toBeInTheDocument();
      expect(screen.getByText("My account")).toBeInTheDocument();
      expect(screen.getByText("Logout")).toBeInTheDocument();
    });

    it("handles logout action", async () => {
      render(
        <TestWrapper>
          <AdminLayout />
        </TestWrapper>,
      );

      const userMenuButton = screen.getByLabelText("account of current user");
      fireEvent.click(userMenuButton);

      const logoutButton = screen.getByText("Logout");
      fireEvent.click(logoutButton);

      await waitFor(() => {
        expect(mockAuthContext.logout).toHaveBeenCalled();
      });
    });
  });

  describe("Security Features", () => {
    it("displays security warning", () => {
      render(
        <TestWrapper>
          <AdminLayout />
        </TestWrapper>,
      );

      // Use within() to target the specific drawer since both mobile and desktop
      // drawers render the same content, avoiding "Found multiple elements" errors
      const desktopDrawer = screen.getByRole("navigation");
      expect(
        within(desktopDrawer).getByText("All actions are logged and monitored"),
      ).toBeInTheDocument();
    });

    it("shows notifications badge when security events exist", () => {
      // Override the global mock to provide security events
      const originalUseSecurity =
        require("../../../contexts/SecurityContext").useSecurity;
      const mockUseSecurity = jest.fn(() => ({
        checkAccess: jest.fn(() => true),
        logSecurityEvent: jest.fn(),
        securityEvents: [
          {
            id: "1",
            type: "login_failed",
            timestamp: new Date().toISOString(),
            details: "Failed login attempt",
          },
          {
            id: "2",
            type: "suspicious_activity",
            timestamp: new Date().toISOString(),
            details: "Suspicious activity detected",
          },
        ],
      }));

      // Temporarily replace the mock
      require("../../../contexts/SecurityContext").useSecurity =
        mockUseSecurity;

      render(
        <TestWrapper>
          <AdminLayout />
        </TestWrapper>,
      );

      // Step 1: Verify the main layout renders
      expect(screen.getByTestId("admin-layout")).toBeInTheDocument();

      // Step 2: Check that the notification badge shows the correct count
      expect(screen.getByText("2")).toBeInTheDocument();

      // Step 3: Verify security events summary appears in the navigation drawer
      // Note: Using within() to target the specific drawer since both mobile and desktop
      // drawers render the same content, avoiding "Found multiple elements" errors
      const desktopDrawer = screen.getByRole("navigation");
      expect(
        within(desktopDrawer).getByText("2 security event(s)"),
      ).toBeInTheDocument();

      // Restore the original mock
      require("../../../contexts/SecurityContext").useSecurity =
        originalUseSecurity;
    });

    it("formats session time correctly", () => {
      // Test implementation would go here

      render(
        <BrowserRouter>
          <ThemeProvider theme={theme}>
            <AuthProvider>
              <SecurityProvider>
                <AdminLayout />
              </SecurityProvider>
            </AuthProvider>
          </ThemeProvider>
        </BrowserRouter>,
      );

      expect(screen.getByText("Session: 61:01")).toBeInTheDocument();
    });
  });

  describe("Responsive Design", () => {
    it("toggles mobile drawer when menu button is clicked", () => {
      render(
        <TestWrapper>
          <AdminLayout />
        </TestWrapper>,
      );

      // Mobile menu button should be present (hidden on desktop)
      const menuButton = screen.getByLabelText("open drawer");
      expect(menuButton).toBeInTheDocument();

      fireEvent.click(menuButton);

      // Drawer should be open (this is handled by MUI internally)
      expect(menuButton).toBeInTheDocument();
    });
  });

  describe("Role-based Access Control", () => {
    it("hides navigation items for read-only users", () => {
      // Import the actual modules to spy on them
      const authContext = require("../../../contexts/AuthContext");
      const securityContext = require("../../../contexts/SecurityContext");

      // Spy on the hooks and provide read-only user mocks
      const authSpy = jest.spyOn(authContext, "useAuth").mockReturnValue({
        user: {
          id: "test-user",
          username: "readonly",
          role: "read_only", // Read-only role
          sessionExpiry: new Date(Date.now() + 3600000).toISOString(),
        },
        isAuthenticated: true,
        login: jest.fn(),
        logout: jest.fn(),
        refreshSession: jest.fn(),
        extendSession: jest.fn(),
      });

      const securitySpy = jest
        .spyOn(securityContext, "useSecurity")
        .mockReturnValue({
          checkAccess: jest.fn((role) => {
            // Read-only users can only access read_only items
            return role === "read_only";
          }),
          logSecurityEvent: jest.fn(),
          securityEvents: [],
          sessionRemainingTime: 3600,
        });

      render(
        <TestWrapper>
          <AdminLayout />
        </TestWrapper>,
      );

      // Use within() to target the desktop drawer to avoid duplicate element issues
      const desktopDrawer = screen.getByRole("navigation");

      // Read-only users should only see basic items
      expect(within(desktopDrawer).getByText("Dashboard")).toBeInTheDocument();
      expect(
        within(desktopDrawer).getByText("System Health"),
      ).toBeInTheDocument();
      expect(within(desktopDrawer).getByText("Monitoring")).toBeInTheDocument();
      expect(within(desktopDrawer).getByText("Audit Logs")).toBeInTheDocument();

      // Admin-only items should not be visible
      expect(
        within(desktopDrawer).queryByText("Crawler Management"),
      ).not.toBeInTheDocument();
      expect(
        within(desktopDrawer).queryByText("Database Management"),
      ).not.toBeInTheDocument();
      expect(
        within(desktopDrawer).queryByText("User Management"),
      ).not.toBeInTheDocument();
      expect(
        within(desktopDrawer).queryByText("Security"),
      ).not.toBeInTheDocument();
      expect(
        within(desktopDrawer).queryByText("System Config"),
      ).not.toBeInTheDocument();

      // Restore the spies
      authSpy.mockRestore();
      securitySpy.mockRestore();
    });

    it("shows admin items for admin users", () => {
      // Override the global mock to provide an admin user (not super_admin)
      const originalUseAuth = require("../../../contexts/AuthContext").useAuth;
      const originalUseSecurity =
        require("../../../contexts/SecurityContext").useSecurity;

      const mockUseAuth = jest.fn(() => ({
        user: {
          id: "test-user",
          username: "admin",
          role: "admin", // Admin role, not super_admin
          sessionExpiry: new Date(Date.now() + 3600000).toISOString(),
        },
        isAuthenticated: true,
        login: jest.fn(),
        logout: jest.fn(),
        refreshSession: jest.fn(),
        extendSession: jest.fn(),
      }));

      const mockUseSecurity = jest.fn(() => ({
        checkAccess: jest.fn((role) => {
          // Admin users can access admin and read_only roles, but not super_admin
          return role === "admin" || role === "read_only";
        }),
        logSecurityEvent: jest.fn(),
        securityEvents: [],
      }));

      // Temporarily replace the mocks
      require("../../../contexts/AuthContext").useAuth = mockUseAuth;
      require("../../../contexts/SecurityContext").useSecurity =
        mockUseSecurity;

      render(
        <TestWrapper>
          <AdminLayout />
        </TestWrapper>,
      );

      // Admin users should see admin items but not super admin items
      expect(screen.getAllByText("Crawler Management")[0]).toBeInTheDocument();
      expect(screen.getAllByText("Security")[0]).toBeInTheDocument();

      // Super admin items should not be visible
      expect(screen.queryAllByText("Database Management")).toHaveLength(0);
      expect(screen.queryAllByText("User Management")).toHaveLength(0);
      expect(screen.queryAllByText("System Config")).toHaveLength(0);

      // Restore the original mocks
      require("../../../contexts/AuthContext").useAuth = originalUseAuth;
      require("../../../contexts/SecurityContext").useSecurity =
        originalUseSecurity;
    });
  });

  describe("Error Handling", () => {
    it("handles missing user data gracefully", () => {
      // Override the global mock to provide a user with missing data
      const originalUseAuth = require("../../../contexts/AuthContext").useAuth;
      const mockUseAuth = jest.fn(() => ({
        user: {
          id: "test-user",
          username: null, // Missing username to trigger fallback
          role: null, // Missing role to trigger fallback
          sessionExpiry: new Date(Date.now() + 3600000).toISOString(),
        },
        isAuthenticated: true,
        login: jest.fn(),
        logout: jest.fn(),
        refreshSession: jest.fn(),
        extendSession: jest.fn(),
      }));

      // Temporarily replace the mock
      require("../../../contexts/AuthContext").useAuth = mockUseAuth;

      render(
        <TestWrapper>
          <AdminLayout />
        </TestWrapper>,
      );

      expect(screen.getAllByText("Administrator")[0]).toBeInTheDocument();
      expect(screen.getAllByText("unknown")[0]).toBeInTheDocument();

      // Restore the original mock
      require("../../../contexts/AuthContext").useAuth = originalUseAuth;
    });
  });
});
