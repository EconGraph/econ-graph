// REQUIREMENT: Secure administrative interface layout with navigation and security indicators
// PURPOSE: Provide consistent admin layout with role-based navigation and security warnings
// This ensures administrators have clear visual indicators and proper access controls

import React, { useState } from "react";
import { Outlet, useNavigate, useLocation } from "react-router-dom";
import {
  Box,
  Drawer,
  AppBar,
  Toolbar,
  Typography,
  List,
  ListItem,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  IconButton,
  Badge,
  Alert,
  Chip,
  Avatar,
  Menu,
  MenuItem,
  Divider,
} from "@mui/material";
import {
  Menu as MenuIcon,
  Dashboard,
  HealthAndSafety,
  Monitor,
  Storage,
  People,
  Security,
  Assignment,
  Settings,
  Notifications,
  AccountCircle,
  Logout,
  Warning,
} from "@mui/icons-material";
import { useAuth } from "../../contexts/AuthContext";
import { useSecurity } from "../../contexts/SecurityContext";

const DRAWER_WIDTH = 280;

interface NavigationItem {
  label: string;
  path: string;
  icon: React.ReactElement;
  requiredRole: string;
  badge?: number;
}

const navigationItems: NavigationItem[] = [
  {
    label: "Dashboard",
    path: "/",
    icon: <Dashboard />,
    requiredRole: "read_only",
  },
  {
    label: "System Health",
    path: "/health",
    icon: <HealthAndSafety />,
    requiredRole: "read_only",
  },
  {
    label: "Monitoring",
    path: "/monitoring",
    icon: <Monitor />,
    requiredRole: "read_only",
  },
  {
    label: "Crawler Management",
    path: "/crawler",
    icon: <Storage />,
    requiredRole: "admin",
  },
  {
    label: "Database Management",
    path: "/database",
    icon: <Storage />,
    requiredRole: "super_admin",
  },
  {
    label: "User Management",
    path: "/users",
    icon: <People />,
    requiredRole: "super_admin",
    badge: 0, // Will be updated with active user count
  },
  {
    label: "Security",
    path: "/security",
    icon: <Security />,
    requiredRole: "admin",
  },
  {
    label: "Audit Logs",
    path: "/audit",
    icon: <Assignment />,
    requiredRole: "read_only",
  },
  {
    label: "System Config",
    path: "/config",
    icon: <Settings />,
    requiredRole: "super_admin",
  },
];

export default function AdminLayout() {
  const [mobileOpen, setMobileOpen] = useState(false);
  const [anchorEl, setAnchorEl] = useState<null | HTMLElement>(null);
  const navigate = useNavigate();
  const location = useLocation();
  const { user, logout } = useAuth();
  const { checkAccess, sessionRemainingTime, securityEvents } = useSecurity();

  const handleDrawerToggle = () => {
    setMobileOpen(!mobileOpen);
  };

  const handleProfileMenuOpen = (event: React.MouseEvent<HTMLElement>) => {
    setAnchorEl(event.currentTarget);
  };

  const handleProfileMenuClose = () => {
    setAnchorEl(null);
  };

  const handleLogout = async () => {
    handleProfileMenuClose();
    await logout();
  };

  const formatSessionTime = (seconds: number) => {
    const minutes = Math.floor(seconds / 60);
    const remainingSeconds = seconds % 60;
    return `${minutes}:${remainingSeconds.toString().padStart(2, "0")}`;
  };

  const getRoleColor = (role: string) => {
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
  };

  const drawer = (
    <Box>
      {/* Security Warning */}
      <Alert severity="warning" sx={{ m: 2, mb: 1 }}>
        <Typography variant="caption" display="block">
          <strong>🔒 ADMIN INTERFACE</strong>
        </Typography>
        <Typography variant="caption" display="block">
          All actions are logged and monitored
        </Typography>
      </Alert>

      {/* User Info */}
      <Box sx={{ p: 2, borderBottom: 1, borderColor: "divider" }}>
        <Box sx={{ display: "flex", alignItems: "center", mb: 1 }}>
          <Avatar sx={{ bgcolor: "primary.main", mr: 1 }}>
            {user?.username?.charAt(0) || "A"}
          </Avatar>
          <Box>
            <Typography variant="subtitle2" noWrap>
              {user?.username || "Administrator"}
            </Typography>
            <Chip
              label={user?.role || "unknown"}
              size="small"
              color={getRoleColor(user?.role || "")}
              variant="outlined"
            />
          </Box>
        </Box>
        <Typography variant="caption" color="text.secondary">
          Session: {formatSessionTime(sessionRemainingTime)}
        </Typography>
      </Box>

      {/* Navigation */}
      <List>
        {navigationItems.map((item) => {
          if (!checkAccess(item.requiredRole)) {
            return null;
          }

          const isActive = location.pathname === item.path;

          return (
            <ListItem key={item.path} disablePadding>
              <ListItemButton
                selected={isActive}
                onClick={() => {
                  navigate(item.path);
                  setMobileOpen(false);
                }}
                sx={{
                  "&.Mui-selected": {
                    backgroundColor: "primary.main",
                    color: "primary.contrastText",
                    "&:hover": {
                      backgroundColor: "primary.dark",
                    },
                    "& .MuiListItemIcon-root": {
                      color: "primary.contrastText",
                    },
                  },
                }}
              >
                <ListItemIcon>
                  {item.badge !== undefined ? (
                    <Badge badgeContent={item.badge} color="error">
                      {item.icon}
                    </Badge>
                  ) : (
                    item.icon
                  )}
                </ListItemIcon>
                <ListItemText primary={item.label} />
              </ListItemButton>
            </ListItem>
          );
        })}
      </List>

      {/* Security Events Summary */}
      {securityEvents.length > 0 && (
        <Box sx={{ p: 2, borderTop: 1, borderColor: "divider" }}>
          <Typography variant="caption" color="error" display="block">
            <Warning
              fontSize="small"
              sx={{ mr: 0.5, verticalAlign: "middle" }}
            />
            {securityEvents.length} security event(s)
          </Typography>
        </Box>
      )}
    </Box>
  );

  return (
    <Box sx={{ display: "flex" }}>
      {/* App Bar */}
      <AppBar
        position="fixed"
        sx={{
          width: { sm: `calc(100% - ${DRAWER_WIDTH}px)` },
          ml: { sm: `${DRAWER_WIDTH}px` },
          backgroundColor: "#d32f2f", // Admin red theme
        }}
      >
        <Toolbar>
          <IconButton
            color="inherit"
            aria-label="open drawer"
            edge="start"
            onClick={handleDrawerToggle}
            sx={{ mr: 2, display: { sm: "none" } }}
          >
            <MenuIcon />
          </IconButton>

          <Typography variant="h6" noWrap component="div" sx={{ flexGrow: 1 }}>
            EconGraph Administration
          </Typography>

          {/* Notifications */}
          <IconButton color="inherit" sx={{ mr: 1 }}>
            <Badge badgeContent={securityEvents.length} color="error">
              <Notifications />
            </Badge>
          </IconButton>

          {/* User Menu */}
          <IconButton
            size="large"
            edge="end"
            aria-label="account of current user"
            aria-controls="primary-search-account-menu"
            aria-haspopup="true"
            onClick={handleProfileMenuOpen}
            color="inherit"
          >
            <AccountCircle />
          </IconButton>
        </Toolbar>
      </AppBar>

      {/* User Profile Menu */}
      <Menu
        anchorEl={anchorEl}
        open={Boolean(anchorEl)}
        onClose={handleProfileMenuClose}
        onClick={handleProfileMenuClose}
        PaperProps={{
          elevation: 0,
          sx: {
            overflow: "visible",
            filter: "drop-shadow(0px 2px 8px rgba(0,0,0,0.32))",
            mt: 1.5,
            "& .MuiAvatar-root": {
              width: 32,
              height: 32,
              ml: -0.5,
              mr: 1,
            },
            "&:before": {
              content: '""',
              display: "block",
              position: "absolute",
              top: 0,
              right: 14,
              width: 10,
              height: 10,
              bgcolor: "background.paper",
              transform: "translateY(-50%) rotate(45deg)",
              zIndex: 0,
            },
          },
        }}
        transformOrigin={{ horizontal: "right", vertical: "top" }}
        anchorOrigin={{ horizontal: "right", vertical: "bottom" }}
      >
        <MenuItem onClick={handleProfileMenuClose}>
          <Avatar /> Profile
        </MenuItem>
        <MenuItem onClick={handleProfileMenuClose}>
          <Avatar /> My account
        </MenuItem>
        <Divider />
        <MenuItem onClick={handleLogout}>
          <ListItemIcon>
            <Logout fontSize="small" />
          </ListItemIcon>
          Logout
        </MenuItem>
      </Menu>

      {/* Navigation Drawer */}
      <Box
        component="nav"
        sx={{ width: { sm: DRAWER_WIDTH }, flexShrink: { sm: 0 } }}
      >
        <Drawer
          variant="temporary"
          open={mobileOpen}
          onClose={handleDrawerToggle}
          ModalProps={{
            keepMounted: true, // Better open performance on mobile.
          }}
          sx={{
            display: { xs: "block", sm: "none" },
            "& .MuiDrawer-paper": {
              boxSizing: "border-box",
              width: DRAWER_WIDTH,
            },
          }}
        >
          {drawer}
        </Drawer>
        <Drawer
          variant="permanent"
          sx={{
            display: { xs: "none", sm: "block" },
            "& .MuiDrawer-paper": {
              boxSizing: "border-box",
              width: DRAWER_WIDTH,
            },
          }}
          open
        >
          {drawer}
        </Drawer>
      </Box>

      {/* Main Content */}
      <Box
        component="main"
        sx={{
          flexGrow: 1,
          p: 3,
          width: { sm: `calc(100% - ${DRAWER_WIDTH}px)` },
        }}
      >
        <Toolbar />
        <Outlet />
      </Box>
    </Box>
  );
}
