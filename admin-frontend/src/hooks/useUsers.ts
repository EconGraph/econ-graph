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

// GraphQL API functions for user management operations

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
      const response = await fetch("/graphql", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          query: `
            query GetUsers($role: String, $status: String, $search: String) {
              users(role: $role, status: $status, search: $search) {
                id
                name
                email
                role
                status
                lastLogin
                createdAt
                isOnline
                sessionId
                ipAddress
                userAgent
              }
            }
          `,
          variables: {
            role: filters?.role !== "all" ? filters?.role : undefined,
            status: filters?.status !== "all" ? filters?.status : undefined,
            search: filters?.search || undefined,
          },
        }),
      });

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }

      const result = await response.json();

      if (result.errors) {
        throw new Error(result.errors[0].message);
      }

      return result.data.users;
    },
    staleTime: 2 * 60 * 1000, // 2 minutes for user data
    gcTime: 5 * 60 * 1000, // 5 minutes cache
  });
};

/**
 * Fetch online users only
 */
export const useOnlineUsers = () => {
  // Disable refetch interval in test environments to prevent infinite loops
  const isTestEnv = process.env.NODE_ENV === "test" || process.env.VITEST;

  return useQuery({
    queryKey: ["users", "online"],
    queryFn: async (): Promise<User[]> => {
      const response = await fetch("/graphql", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          query: `
            query GetOnlineUsers {
              onlineUsers {
                id
                name
                email
                role
                status
                lastLogin
                createdAt
                isOnline
                sessionId
                ipAddress
                userAgent
              }
            }
          `,
        }),
      });

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }

      const result = await response.json();

      if (result.errors) {
        throw new Error(result.errors[0].message);
      }

      return result.data.onlineUsers;
    },
    staleTime: 30 * 1000, // 30 seconds for online status (changes frequently)
    gcTime: 2 * 60 * 1000, // 2 minutes cache
    refetchInterval: isTestEnv ? false : 30 * 1000, // Disable refetch in tests to prevent infinite loops
  });
};

/**
 * Fetch a single user by ID
 */
export const useUser = (userId: string) => {
  return useQuery({
    queryKey: ["users", userId],
    queryFn: async (): Promise<User> => {
      const response = await fetch("/graphql", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          query: `
            query GetUser($id: ID!) {
              user(id: $id) {
                id
                name
                email
                role
                status
                lastLogin
                createdAt
                isOnline
                sessionId
                ipAddress
                userAgent
              }
            }
          `,
          variables: { id: userId },
        }),
      });

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }

      const result = await response.json();

      if (result.errors) {
        throw new Error(result.errors[0].message);
      }

      if (!result.data.user) {
        throw new Error(`User with ID ${userId} not found`);
      }

      return result.data.user;
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
      const response = await fetch("/graphql", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          query: `
            mutation CreateUser($input: CreateUserInput!) {
              createUser(input: $input) {
                id
                name
                email
                role
                status
                lastLogin
                createdAt
                isOnline
                sessionId
                ipAddress
                userAgent
              }
            }
          `,
          variables: {
            input: userData,
          },
        }),
      });

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }

      const result = await response.json();

      if (result.errors) {
        throw new Error(result.errors[0].message);
      }

      return result.data.createUser;
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
      const response = await fetch("/graphql", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          query: `
            mutation UpdateUser($id: ID!, $input: UpdateUserInput!) {
              updateUser(id: $id, input: $input) {
                id
                name
                email
                role
                status
                lastLogin
                createdAt
                isOnline
                sessionId
                ipAddress
                userAgent
              }
            }
          `,
          variables: {
            id,
            input: userData,
          },
        }),
      });

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }

      const result = await response.json();

      if (result.errors) {
        throw new Error(result.errors[0].message);
      }

      return result.data.updateUser;
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
      const response = await fetch("/graphql", {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          query: `
            mutation DeleteUser($id: ID!) {
              deleteUser(id: $id)
            }
          `,
          variables: { id: userId },
        }),
      });

      if (!response.ok) {
        throw new Error(`HTTP error! status: ${response.status}`);
      }

      const result = await response.json();

      if (result.errors) {
        throw new Error(result.errors[0].message);
      }

      return result.data.deleteUser;
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
      // Invalidate the cache to force refetch
      queryClient.invalidateQueries({ queryKey: ["users"] });
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["users"] });
    },
  });
};
