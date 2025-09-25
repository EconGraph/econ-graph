/**
 * React Query hooks for User Management
 *
 * Provides data fetching, caching, and state management for user operations.
 * Optimized for admin UI with focus on developer experience and debugging.
 */

import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";

export interface User {
  id: string;
  name: string;
  email: string;
  role: "read_only" | "admin" | "super_admin";
  status: "active" | "inactive" | "suspended";
  lastLogin: string;
  createdAt: string;
  isOnline: boolean;
  sessionId?: string;
  ipAddress?: string;
  userAgent?: string;
}

// Mock API functions - in real implementation, these would make actual API calls
const mockUsers: User[] = [
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
  {
    id: "5",
    name: "Charlie Viewer",
    email: "charlie.viewer@company.com",
    role: "read_only",
    status: "suspended",
    lastLogin: "2024-01-05T11:30:00Z",
    createdAt: "2023-10-15T00:00:00Z",
    isOnline: false,
  },
];

// Simulate API delay for realistic testing
const delay = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/**
 * Fetch all users with optional filtering
 */
export const useUsers = (filters?: {
  role?: string;
  status?: string;
  search?: string;
}) => {
  return useQuery({
    queryKey: ["users", filters],
    queryFn: async (): Promise<User[]> => {
      // Simulate API delay
      await delay(500);

      let filteredUsers = [...mockUsers];

      // Apply filters
      if (filters?.role && filters.role !== "all") {
        filteredUsers = filteredUsers.filter(
          (user) => user.role === filters.role,
        );
      }

      if (filters?.status && filters.status !== "all") {
        filteredUsers = filteredUsers.filter(
          (user) => user.status === filters.status,
        );
      }

      if (filters?.search) {
        const searchLower = filters.search.toLowerCase();
        filteredUsers = filteredUsers.filter(
          (user) =>
            user.name.toLowerCase().includes(searchLower) ||
            user.email.toLowerCase().includes(searchLower),
        );
      }

      return filteredUsers;
    },
    staleTime: 2 * 60 * 1000, // 2 minutes for user data
    gcTime: 5 * 60 * 1000, // 5 minutes cache
  });
};

/**
 * Fetch online users only
 */
export const useOnlineUsers = () => {
  return useQuery({
    queryKey: ["users", "online"],
    queryFn: async (): Promise<User[]> => {
      // Simulate API delay
      await delay(300);

      return mockUsers.filter((user) => user.isOnline);
    },
    staleTime: 30 * 1000, // 30 seconds for online status (changes frequently)
    gcTime: 2 * 60 * 1000, // 2 minutes cache
    refetchInterval: 30 * 1000, // Refetch every 30 seconds for real-time updates
  });
};

/**
 * Fetch a single user by ID
 */
export const useUser = (userId: string) => {
  return useQuery({
    queryKey: ["users", userId],
    queryFn: async (): Promise<User> => {
      // Simulate API delay
      await delay(200);

      const user = mockUsers.find((u) => u.id === userId);
      if (!user) {
        throw new Error(`User with ID ${userId} not found`);
      }

      return user;
    },
    enabled: !!userId,
    staleTime: 5 * 60 * 1000, // 5 minutes for individual user data
  });
};

/**
 * Create a new user
 */
export const useCreateUser = () => {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (
      userData: Omit<User, "id" | "createdAt" | "lastLogin">,
    ): Promise<User> => {
      // Simulate API delay
      await delay(800);

      const newUser: User = {
        ...userData,
        id: Date.now().toString(),
        createdAt: new Date().toISOString(),
        lastLogin: new Date().toISOString(),
      };

      // In real implementation, this would make an API call
      mockUsers.push(newUser);

      return newUser;
    },
    onSuccess: () => {
      // Invalidate and refetch users list
      queryClient.invalidateQueries({ queryKey: ["users"] });
    },
  });
};

/**
 * Update an existing user
 */
export const useUpdateUser = () => {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({
      id,
      ...userData
    }: Partial<User> & { id: string }): Promise<User> => {
      // Simulate API delay
      await delay(600);

      const userIndex = mockUsers.findIndex((u) => u.id === id);
      if (userIndex === -1) {
        throw new Error(`User with ID ${id} not found`);
      }

      // Update the user
      mockUsers[userIndex] = { ...mockUsers[userIndex], ...userData };

      return mockUsers[userIndex];
    },
    onSuccess: (updatedUser) => {
      // Invalidate and refetch users list
      queryClient.invalidateQueries({ queryKey: ["users"] });
      // Update the specific user in cache
      queryClient.setQueryData(["users", updatedUser.id], updatedUser);
    },
  });
};

/**
 * Delete a user
 */
export const useDeleteUser = () => {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (userId: string): Promise<void> => {
      // Simulate API delay
      await delay(400);

      const userIndex = mockUsers.findIndex((u) => u.id === userId);
      if (userIndex === -1) {
        throw new Error(`User with ID ${userId} not found`);
      }

      // Remove the user
      mockUsers.splice(userIndex, 1);
    },
    onSuccess: () => {
      // Invalidate and refetch users list
      queryClient.invalidateQueries({ queryKey: ["users"] });
    },
  });
};

/**
 * Refresh users data (useful for manual refresh buttons)
 */
export const useRefreshUsers = () => {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (): Promise<void> => {
      // Simulate API delay
      await delay(300);

      // In real implementation, this might trigger a background sync
      // For now, just invalidate the cache to force refetch
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["users"] });
    },
  });
};
