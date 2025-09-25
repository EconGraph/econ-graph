// REQUIREMENT: User management interface for super_admin role
// PURPOSE: Manage all registered users, view active sessions, and control user access
// This provides comprehensive user administration capabilities for system administrators
//
// UPDATED: Now uses React Query for data fetching, caching, and state management
// This improves testability, performance, and developer experience

import React, { useState, useCallback, useMemo } from "react";
import {
  Box,
  Typography,
  Paper,
  Tabs,
  Tab,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Chip,
  IconButton,
  Button,
  Dialog,
  DialogTitle,
  DialogContent,
  DialogActions,
  TextField,
  FormControl,
  InputLabel,
  Select,
  MenuItem,
  Alert,
  Grid,
  Card,
  CardContent,
  Avatar,
  Tooltip,
  Badge,
  CircularProgress,
} from "@mui/material";
import { PersonAdd, Edit, Delete, Refresh, Search } from "@mui/icons-material";
import { useAuth } from "../contexts/AuthContext";
import { useSecurity } from "../contexts/SecurityContext";
import {
  useUsers,
  useOnlineUsers,
  useCreateUser,
  useUpdateUser,
  useDeleteUser,
  useRefreshUsers,
  User,
} from "../hooks/useUsers";

interface TabPanelProps {
  children?: React.ReactNode;
  index: number;
  value: number;
}

function TabPanel(props: TabPanelProps) {
  const { children, value, index, ...other } = props;

  return (
    <div
      role="tabpanel"
      hidden={value !== index}
      id={`user-tabpanel-${index}`}
      aria-labelledby={`user-tab-${index}`}
      {...other}
    >
      {value === index && <Box sx={{ p: 3 }}>{children}</Box>}
    </div>
  );
}

// Memoized components for expensive date formatting operations
const FormattedDateTime = React.memo(
  ({ dateString }: { dateString: string }) => {
    const formattedDate = useMemo(
      () => new Date(dateString).toLocaleString(),
      [dateString],
    );
    return <span>{formattedDate}</span>;
  },
);

const FormattedDate = React.memo(({ dateString }: { dateString: string }) => {
  const formattedDate = useMemo(
    () => new Date(dateString).toLocaleDateString(),
    [dateString],
  );
  return <span>{formattedDate}</span>;
});

export default function UserManagementPage() {
  const [tabValue, setTabValue] = useState(0);
  const [openDialog, setOpenDialog] = useState(false);
  const [editingUser, setEditingUser] = useState<User | null>(null);
  const [searchTerm, setSearchTerm] = useState("");
  const [roleFilter, setRoleFilter] = useState<string>("all");
  const { user: currentUser } = useAuth();
  const { checkAccess } = useSecurity();

  // React Query hooks for data fetching
  const {
    data: users = [],
    isLoading: usersLoading,
    error: usersError,
  } = useUsers({
    role: roleFilter !== "all" ? roleFilter : undefined,
    search: searchTerm || undefined,
  });

  const { data: onlineUsers = [], isLoading: onlineUsersLoading } =
    useOnlineUsers();

  // Mutations
  const createUserMutation = useCreateUser();
  const updateUserMutation = useUpdateUser();
  const deleteUserMutation = useDeleteUser();
  const refreshUsersMutation = useRefreshUsers();

  // Memoized calculated values
  const totalUsers = useMemo(() => users.length, [users]);
  const activeUsers = useMemo(
    () => users.filter((u) => u.status === "active").length,
    [users],
  );
  const onlineCount = useMemo(() => onlineUsers.length, [onlineUsers]);

  const getStatusColor = useCallback((status: string) => {
    switch (status) {
      case "active":
        return "success";
      case "inactive":
        return "default";
      case "suspended":
        return "error";
      default:
        return "default";
    }
  }, []);

  const getRoleColor = useCallback((role: string) => {
    switch (role) {
      case "super_admin":
        return "error";
      case "admin":
        return "primary";
      case "read_only":
        return "secondary";
      default:
        return "default";
    }
  }, []);

  const handleTabChange = useCallback(
    (event: React.SyntheticEvent, newValue: number) => {
      setTabValue(newValue);
    },
    [],
  );

  const handleCreateUser = useCallback(() => {
    setEditingUser(null);
    setOpenDialog(true);
  }, []);

  const handleEditUser = useCallback((user: User) => {
    setEditingUser(user);
    setOpenDialog(true);
  }, []);

  const handleDeleteUser = useCallback(
    async (userId: string) => {
      if (window.confirm("Are you sure you want to delete this user?")) {
        try {
          await deleteUserMutation.mutateAsync(userId);
        } catch (error) {
          console.error("Failed to delete user:", error);
        }
      }
    },
    [deleteUserMutation],
  );

  const handleSaveUser = useCallback(
    async (userData: Partial<User>) => {
      try {
        if (editingUser) {
          await updateUserMutation.mutateAsync({
            id: editingUser.id,
            ...userData,
          });
        } else {
          await createUserMutation.mutateAsync(
            userData as Omit<User, "id" | "createdAt" | "lastLogin">,
          );
        }
        setOpenDialog(false);
        setEditingUser(null);
      } catch (error) {
        console.error("Failed to save user:", error);
      }
    },
    [editingUser, updateUserMutation, createUserMutation],
  );

  const handleRefresh = useCallback(async () => {
    try {
      await refreshUsersMutation.mutateAsync();
    } catch (error) {
      console.error("Failed to refresh users:", error);
    }
  }, [refreshUsersMutation]);

  // Access control
  if (!checkAccess("super_admin")) {
    return (
      <Box sx={{ p: 3 }}>
        <Alert severity="error">
          Access Denied. This page requires super_admin privileges.
        </Alert>
      </Box>
    );
  }

  return (
    <Box sx={{ p: 3 }}>
      <Typography variant="h4" gutterBottom>
        User Management
      </Typography>

      {/* Summary Cards */}
      <Grid container spacing={3} sx={{ mb: 3 }}>
        <Grid item xs={12} sm={6} md={3}>
          <Card>
            <CardContent>
              <Typography color="textSecondary" gutterBottom>
                Total Users
              </Typography>
              <Typography variant="h4">
                {usersLoading ? <CircularProgress size={24} /> : totalUsers}
              </Typography>
            </CardContent>
          </Card>
        </Grid>
        <Grid item xs={12} sm={6} md={3}>
          <Card>
            <CardContent>
              <Typography color="textSecondary" gutterBottom>
                Active Users
              </Typography>
              <Typography variant="h4">
                {usersLoading ? <CircularProgress size={24} /> : activeUsers}
              </Typography>
            </CardContent>
          </Card>
        </Grid>
        <Grid item xs={12} sm={6} md={3}>
          <Card>
            <CardContent>
              <Typography color="textSecondary" gutterBottom>
                Online Now
              </Typography>
              <Typography variant="h4">
                {onlineUsersLoading ? (
                  <CircularProgress size={24} />
                ) : (
                  onlineCount
                )}
              </Typography>
            </CardContent>
          </Card>
        </Grid>
        <Grid item xs={12} sm={6} md={3}>
          <Card>
            <CardContent>
              <Typography color="textSecondary" gutterBottom>
                Current User
              </Typography>
              <Typography variant="h6">
                {currentUser?.username || "Unknown"}
              </Typography>
            </CardContent>
          </Card>
        </Grid>
      </Grid>

      {/* Error Handling */}
      {usersError && (
        <Alert severity="error" sx={{ mb: 2 }}>
          Failed to load users: {usersError.message}
        </Alert>
      )}

      {/* Controls */}
      <Box sx={{ display: "flex", gap: 2, mb: 3, flexWrap: "wrap" }}>
        <Button
          variant="contained"
          startIcon={<PersonAdd />}
          onClick={handleCreateUser}
          disabled={createUserMutation.isPending}
        >
          Add User
        </Button>
        <Button
          variant="outlined"
          startIcon={<Refresh />}
          onClick={handleRefresh}
          disabled={refreshUsersMutation.isPending}
        >
          Refresh
        </Button>
        <TextField
          placeholder="Search users..."
          value={searchTerm}
          onChange={(e) => setSearchTerm(e.target.value)}
          InputProps={{
            startAdornment: <Search sx={{ mr: 1, color: "text.secondary" }} />,
          }}
          sx={{ minWidth: 200 }}
        />
        <FormControl sx={{ minWidth: 120 }}>
          <InputLabel>Role Filter</InputLabel>
          <Select
            value={roleFilter}
            onChange={(e) => setRoleFilter(e.target.value)}
            label="Role Filter"
          >
            <MenuItem value="all">All Roles</MenuItem>
            <MenuItem value="super_admin">Super Admin</MenuItem>
            <MenuItem value="admin">Admin</MenuItem>
            <MenuItem value="read_only">Read Only</MenuItem>
          </Select>
        </FormControl>
      </Box>

      {/* Tabs */}
      <Paper sx={{ width: "100%" }}>
        <Tabs
          value={tabValue}
          onChange={handleTabChange}
          aria-label="user management tabs"
        >
          <Tab label="All Users" />
          <Tab label="Online Users" />
        </Tabs>

        <TabPanel value={tabValue} index={0}>
          <TableContainer>
            <Table>
              <TableHead>
                <TableRow>
                  <TableCell>User</TableCell>
                  <TableCell>Role</TableCell>
                  <TableCell>Status</TableCell>
                  <TableCell>Last Login</TableCell>
                  <TableCell>Created</TableCell>
                  <TableCell>Actions</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {usersLoading ? (
                  <TableRow>
                    <TableCell colSpan={6} align="center">
                      <CircularProgress />
                    </TableCell>
                  </TableRow>
                ) : (
                  users.map((user) => (
                    <TableRow key={user.id}>
                      <TableCell>
                        <Box
                          sx={{ display: "flex", alignItems: "center", gap: 1 }}
                        >
                          <Avatar sx={{ width: 32, height: 32 }}>
                            {user.name.charAt(0)}
                          </Avatar>
                          <Box>
                            <Typography variant="body2" fontWeight="medium">
                              {user.name}
                            </Typography>
                            <Typography
                              variant="caption"
                              color="text.secondary"
                            >
                              {user.email}
                            </Typography>
                          </Box>
                        </Box>
                      </TableCell>
                      <TableCell>
                        <Chip
                          label={user.role.replace("_", " ")}
                          color={getRoleColor(user.role) as any}
                          size="small"
                        />
                      </TableCell>
                      <TableCell>
                        <Chip
                          label={user.status}
                          color={getStatusColor(user.status) as any}
                          size="small"
                        />
                      </TableCell>
                      <TableCell>
                        <FormattedDateTime dateString={user.lastLogin} />
                      </TableCell>
                      <TableCell>
                        <FormattedDate dateString={user.createdAt} />
                      </TableCell>
                      <TableCell>
                        <Box sx={{ display: "flex", gap: 1 }}>
                          <Tooltip title="Edit User">
                            <IconButton
                              size="small"
                              onClick={() => handleEditUser(user)}
                              disabled={updateUserMutation.isPending}
                            >
                              <Edit />
                            </IconButton>
                          </Tooltip>
                          <Tooltip title="Delete User">
                            <IconButton
                              size="small"
                              onClick={() => handleDeleteUser(user.id)}
                              disabled={deleteUserMutation.isPending}
                            >
                              <Delete />
                            </IconButton>
                          </Tooltip>
                        </Box>
                      </TableCell>
                    </TableRow>
                  ))
                )}
              </TableBody>
            </Table>
          </TableContainer>
        </TabPanel>

        <TabPanel value={tabValue} index={1}>
          <TableContainer>
            <Table>
              <TableHead>
                <TableRow>
                  <TableCell>User</TableCell>
                  <TableCell>Role</TableCell>
                  <TableCell>Session</TableCell>
                  <TableCell>IP Address</TableCell>
                  <TableCell>Last Activity</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {onlineUsersLoading ? (
                  <TableRow>
                    <TableCell colSpan={5} align="center">
                      <CircularProgress />
                    </TableCell>
                  </TableRow>
                ) : (
                  onlineUsers.map((user) => (
                    <TableRow key={user.id}>
                      <TableCell>
                        <Box
                          sx={{ display: "flex", alignItems: "center", gap: 1 }}
                        >
                          <Badge
                            overlap="circular"
                            anchorOrigin={{
                              vertical: "bottom",
                              horizontal: "right",
                            }}
                            variant="dot"
                            color="success"
                          >
                            <Avatar sx={{ width: 32, height: 32 }}>
                              {user.name.charAt(0)}
                            </Avatar>
                          </Badge>
                          <Box>
                            <Typography variant="body2" fontWeight="medium">
                              {user.name}
                            </Typography>
                            <Typography
                              variant="caption"
                              color="text.secondary"
                            >
                              {user.email}
                            </Typography>
                          </Box>
                        </Box>
                      </TableCell>
                      <TableCell>
                        <Chip
                          label={user.role.replace("_", " ")}
                          color={getRoleColor(user.role) as any}
                          size="small"
                        />
                      </TableCell>
                      <TableCell>
                        <Typography variant="caption" fontFamily="monospace">
                          {user.sessionId}
                        </Typography>
                      </TableCell>
                      <TableCell>
                        <Typography variant="caption" fontFamily="monospace">
                          {user.ipAddress}
                        </Typography>
                      </TableCell>
                      <TableCell>
                        <FormattedDateTime dateString={user.lastLogin} />
                      </TableCell>
                    </TableRow>
                  ))
                )}
              </TableBody>
            </Table>
          </TableContainer>
        </TabPanel>
      </Paper>

      {/* Edit/Create User Dialog */}
      <Dialog
        open={openDialog}
        onClose={() => setOpenDialog(false)}
        maxWidth="sm"
        fullWidth
      >
        <DialogTitle>
          {editingUser ? "Edit User" : "Create New User"}
        </DialogTitle>
        <DialogContent>
          <Box sx={{ display: "flex", flexDirection: "column", gap: 2, pt: 1 }}>
            <TextField
              label="Name"
              defaultValue={editingUser?.name || ""}
              fullWidth
            />
            <TextField
              label="Email"
              type="email"
              defaultValue={editingUser?.email || ""}
              fullWidth
            />
            <FormControl fullWidth>
              <InputLabel>Role</InputLabel>
              <Select
                defaultValue={editingUser?.role || "read_only"}
                label="Role"
              >
                <MenuItem value="read_only">Read Only</MenuItem>
                <MenuItem value="admin">Admin</MenuItem>
                <MenuItem value="super_admin">Super Admin</MenuItem>
              </Select>
            </FormControl>
            <FormControl fullWidth>
              <InputLabel>Status</InputLabel>
              <Select
                defaultValue={editingUser?.status || "active"}
                label="Status"
              >
                <MenuItem value="active">Active</MenuItem>
                <MenuItem value="inactive">Inactive</MenuItem>
                <MenuItem value="suspended">Suspended</MenuItem>
              </Select>
            </FormControl>
          </Box>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setOpenDialog(false)}>Cancel</Button>
          <Button
            variant="contained"
            onClick={() => handleSaveUser({})}
            disabled={
              createUserMutation.isPending || updateUserMutation.isPending
            }
          >
            {editingUser ? "Update" : "Create"}
          </Button>
        </DialogActions>
      </Dialog>
    </Box>
  );
}
