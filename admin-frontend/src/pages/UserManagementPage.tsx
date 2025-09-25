// REQUIREMENT: User management interface for super_admin role
// PURPOSE: Manage all registered users, view active sessions, and control user access
// This provides comprehensive user administration capabilities for system administrators

import React, { useState, useEffect, useCallback, useMemo } from "react";
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
} from "@mui/material";
import {
  PersonAdd,
  Edit,
  Delete,
  Block,
  CheckCircle,
  Warning,
  Refresh,
  Search,
  Logout,
} from "@mui/icons-material";
import { useAuth } from "../contexts/AuthContext";
import { useSecurity } from "../contexts/SecurityContext";

interface User {
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
  const [users, setUsers] = useState<User[]>([]);
  const [onlineUsers, setOnlineUsers] = useState<User[]>([]);
  const [, setLoading] = useState(true);
  const [openDialog, setOpenDialog] = useState(false);
  const [editingUser, setEditingUser] = useState<User | null>(null);
  const [searchTerm, setSearchTerm] = useState("");
  const [roleFilter, setRoleFilter] = useState<string>("all");
  const { user: currentUser } = useAuth();
  const { checkAccess } = useSecurity();

  // Mock data - in real implementation, this would come from API
  useEffect(() => {
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
        createdAt: "2023-10-01T00:00:00Z",
        isOnline: false,
      },
      {
        id: "4",
        name: "Alice Developer",
        email: "alice.dev@company.com",
        role: "admin",
        status: "suspended",
        lastLogin: "2024-01-10T14:20:00Z",
        createdAt: "2023-07-20T00:00:00Z",
        isOnline: false,
      },
    ];

    setUsers(mockUsers);
    setOnlineUsers(mockUsers.filter((u) => u.isOnline));
    setLoading(false);
  }, []);

  const handleTabChange = useCallback(
    (event: React.SyntheticEvent, newValue: number) => {
      setTabValue(newValue);
    },
    [],
  );

  const handleEditUser = useCallback((user: User) => {
    setEditingUser(user);
    setOpenDialog(true);
  }, []);

  const handleCloseDialog = useCallback(() => {
    setOpenDialog(false);
    setEditingUser(null);
  }, []);

  const handleSaveUser = useCallback(() => {
    // In real implementation, this would call API to update user
    console.log("Saving user:", editingUser);
    handleCloseDialog();
  }, [editingUser, handleCloseDialog]);

  const handleSuspendUser = useCallback(
    (userId: string) => {
      // In real implementation, this would call API to suspend user
      setUsers(
        users.map((u) =>
          u.id === userId
            ? {
                ...u,
                status: u.status === "suspended" ? "active" : "suspended",
              }
            : u,
        ),
      );
    },
    [users],
  );

  const handleDeleteUser = useCallback(
    (userId: string) => {
      // In real implementation, this would call API to delete user
      setUsers(users.filter((u) => u.id !== userId));
    },
    [users],
  );

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
        return "warning";
      case "read_only":
        return "info";
      default:
        return "default";
    }
  }, []);

  const filteredUsers = useMemo(() => {
    return users.filter((user) => {
      const matchesSearch =
        user.name.toLowerCase().includes(searchTerm.toLowerCase()) ||
        user.email.toLowerCase().includes(searchTerm.toLowerCase());
      const matchesRole = roleFilter === "all" || user.role === roleFilter;
      return matchesSearch && matchesRole;
    });
  }, [users, searchTerm, roleFilter]);

  if (!checkAccess("super_admin")) {
    return (
      <Box sx={{ p: 3, textAlign: "center" }} data-testid="access-denied">
        <Alert severity="error">
          <Typography variant="h6">Access Denied</Typography>
          <Typography>
            You need super_admin privileges to access user management.
          </Typography>
        </Alert>
      </Box>
    );
  }

  return (
    <Box data-testid="user-management-content">
      <Typography variant="h4" gutterBottom>
        User Management
      </Typography>
      <Typography variant="subtitle1" color="text.secondary" gutterBottom>
        Manage registered users, active sessions, and access controls
      </Typography>

      {/* Statistics Cards */}
      <Grid container spacing={3} sx={{ mb: 3 }}>
        <Grid item xs={12} sm={6} md={3}>
          <Card>
            <CardContent>
              <Box sx={{ display: "flex", alignItems: "center" }}>
                <Avatar sx={{ bgcolor: "primary.main", mr: 2 }}>
                  <CheckCircle />
                </Avatar>
                <Box>
                  <Typography variant="h4">{users.length}</Typography>
                  <Typography variant="body2" color="text.secondary">
                    Total Users
                  </Typography>
                </Box>
              </Box>
            </CardContent>
          </Card>
        </Grid>
        <Grid item xs={12} sm={6} md={3}>
          <Card>
            <CardContent>
              <Box sx={{ display: "flex", alignItems: "center" }}>
                <Avatar sx={{ bgcolor: "success.main", mr: 2 }}>
                  <Badge badgeContent={onlineUsers.length} color="error">
                    <CheckCircle />
                  </Badge>
                </Avatar>
                <Box>
                  <Typography variant="h4">{onlineUsers.length}</Typography>
                  <Typography variant="body2" color="text.secondary">
                    Online Now
                  </Typography>
                </Box>
              </Box>
            </CardContent>
          </Card>
        </Grid>
        <Grid item xs={12} sm={6} md={3}>
          <Card>
            <CardContent>
              <Box sx={{ display: "flex", alignItems: "center" }}>
                <Avatar sx={{ bgcolor: "warning.main", mr: 2 }}>
                  <Warning />
                </Avatar>
                <Box>
                  <Typography variant="h4">
                    {users.filter((u) => u.status === "suspended").length}
                  </Typography>
                  <Typography variant="body2" color="text.secondary">
                    Suspended
                  </Typography>
                </Box>
              </Box>
            </CardContent>
          </Card>
        </Grid>
        <Grid item xs={12} sm={6} md={3}>
          <Card>
            <CardContent>
              <Box sx={{ display: "flex", alignItems: "center" }}>
                <Avatar sx={{ bgcolor: "info.main", mr: 2 }}>
                  <PersonAdd />
                </Avatar>
                <Box>
                  <Typography variant="h4">
                    {
                      users.filter(
                        (u) => u.role === "admin" || u.role === "super_admin",
                      ).length
                    }
                  </Typography>
                  <Typography variant="body2" color="text.secondary">
                    Admins
                  </Typography>
                </Box>
              </Box>
            </CardContent>
          </Card>
        </Grid>
      </Grid>

      <Paper sx={{ width: "100%" }}>
        <Box sx={{ borderBottom: 1, borderColor: "divider" }}>
          <Tabs value={tabValue} onChange={handleTabChange}>
            <Tab label="All Users" />
            <Tab label="Online Users" />
            <Tab label="User Activity" />
          </Tabs>
        </Box>

        {/* All Users Tab */}
        <TabPanel value={tabValue} index={0}>
          <Box sx={{ display: "flex", gap: 2, mb: 3 }}>
            <TextField
              placeholder="Search users..."
              value={searchTerm}
              onChange={(e) => setSearchTerm(e.target.value)}
              InputProps={{
                startAdornment: (
                  <Search sx={{ mr: 1, color: "text.secondary" }} />
                ),
              }}
              sx={{ flexGrow: 1 }}
            />
            <FormControl sx={{ minWidth: 120 }}>
              <InputLabel>Role</InputLabel>
              <Select
                value={roleFilter}
                label="Role"
                onChange={(e) => setRoleFilter(e.target.value)}
              >
                <MenuItem value="all">All Roles</MenuItem>
                <MenuItem value="super_admin">Super Admin</MenuItem>
                <MenuItem value="admin">Admin</MenuItem>
                <MenuItem value="read_only">Read Only</MenuItem>
              </Select>
            </FormControl>
            <Button
              variant="contained"
              startIcon={<PersonAdd />}
              onClick={() => setOpenDialog(true)}
            >
              Add User
            </Button>
            <IconButton onClick={() => setLoading(true)} aria-label="refresh">
              <Refresh />
            </IconButton>
          </Box>

          <TableContainer>
            <Table>
              <TableHead>
                <TableRow>
                  <TableCell>User</TableCell>
                  <TableCell>Role</TableCell>
                  <TableCell>Status</TableCell>
                  <TableCell>Last Login</TableCell>
                  <TableCell>Created</TableCell>
                  <TableCell align="right">Actions</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {filteredUsers.map((user) => (
                  <TableRow key={user.id}>
                    <TableCell>
                      <Box sx={{ display: "flex", alignItems: "center" }}>
                        <Avatar sx={{ mr: 2, bgcolor: "primary.main" }}>
                          {user.name.charAt(0)}
                        </Avatar>
                        <Box>
                          <Typography variant="subtitle2">
                            {user.name}
                          </Typography>
                          <Typography variant="body2" color="text.secondary">
                            {user.email}
                          </Typography>
                        </Box>
                      </Box>
                    </TableCell>
                    <TableCell>
                      <Chip
                        label={user.role}
                        color={getRoleColor(user.role)}
                        size="small"
                      />
                    </TableCell>
                    <TableCell>
                      <Chip
                        label={user.status}
                        color={getStatusColor(user.status)}
                        size="small"
                        icon={user.isOnline ? <CheckCircle /> : undefined}
                      />
                    </TableCell>
                    <TableCell>
                      <FormattedDateTime dateString={user.lastLogin} />
                    </TableCell>
                    <TableCell>
                      <FormattedDate dateString={user.createdAt} />
                    </TableCell>
                    <TableCell align="right">
                      <Tooltip title="Edit User">
                        <IconButton
                          size="small"
                          onClick={() => handleEditUser(user)}
                          disabled={user.id === currentUser?.id}
                        >
                          <Edit />
                        </IconButton>
                      </Tooltip>
                      <Tooltip
                        title={
                          user.status === "suspended"
                            ? "Activate User"
                            : "Suspend User"
                        }
                      >
                        <IconButton
                          size="small"
                          onClick={() => handleSuspendUser(user.id)}
                          disabled={user.id === currentUser?.id}
                        >
                          <Block />
                        </IconButton>
                      </Tooltip>
                      <Tooltip title="Delete User">
                        <IconButton
                          size="small"
                          onClick={() => handleDeleteUser(user.id)}
                          disabled={user.id === currentUser?.id}
                          color="error"
                        >
                          <Delete />
                        </IconButton>
                      </Tooltip>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
        </TabPanel>

        {/* Online Users Tab */}
        <TabPanel value={tabValue} index={1}>
          <Typography variant="h6" gutterBottom>
            Currently Online Users ({onlineUsers.length})
          </Typography>
          <TableContainer>
            <Table>
              <TableHead>
                <TableRow>
                  <TableCell>User</TableCell>
                  <TableCell>Role</TableCell>
                  <TableCell>IP Address</TableCell>
                  <TableCell>Session Started</TableCell>
                  <TableCell>User Agent</TableCell>
                  <TableCell align="right">Actions</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {onlineUsers.map((user) => (
                  <TableRow key={user.id}>
                    <TableCell>
                      <Box sx={{ display: "flex", alignItems: "center" }}>
                        <Avatar sx={{ mr: 2, bgcolor: "success.main" }}>
                          {user.name.charAt(0)}
                        </Avatar>
                        <Box>
                          <Typography variant="subtitle2">
                            {user.name}
                          </Typography>
                          <Typography variant="body2" color="text.secondary">
                            {user.email}
                          </Typography>
                        </Box>
                      </Box>
                    </TableCell>
                    <TableCell>
                      <Chip
                        label={user.role}
                        color={getRoleColor(user.role)}
                        size="small"
                      />
                    </TableCell>
                    <TableCell>{user.ipAddress}</TableCell>
                    <TableCell>
                      <FormattedDateTime dateString={user.lastLogin} />
                    </TableCell>
                    <TableCell>
                      <Tooltip title={user.userAgent}>
                        <Typography
                          variant="body2"
                          noWrap
                          sx={{ maxWidth: 200 }}
                        >
                          {user.userAgent?.split(" ")[0]}
                        </Typography>
                      </Tooltip>
                    </TableCell>
                    <TableCell align="right">
                      <Tooltip title="Force Logout">
                        <IconButton
                          size="small"
                          color="error"
                          disabled={user.id === currentUser?.id}
                        >
                          <Logout />
                        </IconButton>
                      </Tooltip>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
        </TabPanel>

        {/* User Activity Tab */}
        <TabPanel value={tabValue} index={2}>
          <Typography variant="h6" gutterBottom>
            Recent User Activity
          </Typography>
          <Alert severity="info">
            User activity logs and audit trails would be displayed here in a
            real implementation. This would include login attempts, permission
            changes, and administrative actions.
          </Alert>
        </TabPanel>
      </Paper>

      {/* Edit User Dialog */}
      <Dialog
        open={openDialog}
        onClose={handleCloseDialog}
        maxWidth="sm"
        fullWidth
      >
        <DialogTitle>{editingUser ? "Edit User" : "Add New User"}</DialogTitle>
        <DialogContent>
          <Box sx={{ pt: 1 }}>
            <TextField
              fullWidth
              label="Name"
              value={editingUser?.name || ""}
              onChange={(e) =>
                setEditingUser((prev) =>
                  prev ? { ...prev, name: e.target.value } : null,
                )
              }
              sx={{ mb: 2 }}
            />
            <TextField
              fullWidth
              label="Email"
              type="email"
              value={editingUser?.email || ""}
              onChange={(e) =>
                setEditingUser((prev) =>
                  prev ? { ...prev, email: e.target.value } : null,
                )
              }
              sx={{ mb: 2 }}
            />
            <FormControl fullWidth sx={{ mb: 2 }}>
              <InputLabel>Role</InputLabel>
              <Select
                value={editingUser?.role || "read_only"}
                label="Role"
                onChange={(e) =>
                  setEditingUser((prev) =>
                    prev ? { ...prev, role: e.target.value as any } : null,
                  )
                }
              >
                <MenuItem value="read_only">Read Only</MenuItem>
                <MenuItem value="admin">Admin</MenuItem>
                <MenuItem value="super_admin">Super Admin</MenuItem>
              </Select>
            </FormControl>
            <FormControl fullWidth>
              <InputLabel>Status</InputLabel>
              <Select
                value={editingUser?.status || "active"}
                label="Status"
                onChange={(e) =>
                  setEditingUser((prev) =>
                    prev ? { ...prev, status: e.target.value as any } : null,
                  )
                }
              >
                <MenuItem value="active">Active</MenuItem>
                <MenuItem value="inactive">Inactive</MenuItem>
                <MenuItem value="suspended">Suspended</MenuItem>
              </Select>
            </FormControl>
          </Box>
        </DialogContent>
        <DialogActions>
          <Button onClick={handleCloseDialog}>Cancel</Button>
          <Button onClick={handleSaveUser} variant="contained">
            Save
          </Button>
        </DialogActions>
      </Dialog>
    </Box>
  );
}
