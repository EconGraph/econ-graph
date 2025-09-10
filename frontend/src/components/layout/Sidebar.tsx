import React from 'react';
import { useNavigate, useLocation } from 'react-router-dom';
import {
  Drawer,
  List,
  ListItem,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Divider,
  Box,
  Typography,
  useTheme,
  useMediaQuery,
} from '@mui/material';
import {
  Dashboard as DashboardIcon,
  Explore as ExploreIcon,
  DataObject as DataSourceIcon,
  Info as InfoIcon,
  TrendingUp as TrendingUpIcon,
  // Assessment as AssessmentIcon, // Unused but kept for future features
  Public as GlobalIcon,
} from '@mui/icons-material';

const drawerWidth = 240;

interface SidebarProps {
  open: boolean;
  onClose: () => void;
}

interface NavigationItem {
  text: string;
  path: string;
  icon: React.ReactElement;
  description: string;
}

/**
 * REQUIREMENT: Modern navigation that's easier to use than FRED
 * PURPOSE: Responsive sidebar navigation with clear categorization
 * This provides intuitive navigation structure that improves on FRED's menu system
 */
const Sidebar: React.FC<SidebarProps> = ({ open, onClose }) => {
  const navigate = useNavigate();
  const location = useLocation();
  const theme = useTheme();
  const isMobile = useMediaQuery(theme.breakpoints.down('sm'));

  // REQUIREMENT: Function similarly to FRED but with better organization
  const navigationItems: NavigationItem[] = [
    {
      text: 'Dashboard',
      path: '/',
      icon: <DashboardIcon />,
      description: 'Overview and recent data',
    },
    {
      text: 'Explore Series',
      path: '/explore',
      icon: <ExploreIcon />,
      description: 'Browse and search economic data',
    },
    {
      text: 'Global Analysis',
      path: '/global',
      icon: <GlobalIcon />,
      description: 'Cross-country correlations & network analysis',
    },
    {
      text: 'Data Sources',
      path: '/sources',
      icon: <DataSourceIcon />,
      description: 'FRED, BLS, and other sources',
    },
  ];

  const secondaryItems: NavigationItem[] = [
    {
      text: 'About',
      path: '/about',
      icon: <InfoIcon />,
      description: 'About EconGraph',
    },
  ];

  const handleNavigation = (path: string) => {
    navigate(path);
    if (isMobile) {
      onClose();
    }
  };

  const renderNavigationItems = (items: NavigationItem[], showDivider = false) => (
    <>
      {showDivider && <Divider sx={{ my: 1 }} />}
      {items.map(item => (
        <ListItem key={item.path} disablePadding>
          <ListItemButton
            selected={location.pathname === item.path}
            onClick={() => handleNavigation(item.path)}
            sx={{
              minHeight: 48,
              px: 2.5,
              '&.Mui-selected': {
                backgroundColor: theme.palette.primary.main,
                color: theme.palette.primary.contrastText,
                '&:hover': {
                  backgroundColor: theme.palette.primary.dark,
                },
                '& .MuiListItemIcon-root': {
                  color: theme.palette.primary.contrastText,
                },
              },
            }}
          >
            <ListItemIcon
              sx={{
                minWidth: 0,
                mr: 3,
                justifyContent: 'center',
              }}
            >
              {item.icon}
            </ListItemIcon>
            <ListItemText
              primary={item.text}
              secondary={item.description}
              primaryTypographyProps={{
                fontSize: '0.875rem',
                fontWeight: location.pathname === item.path ? 600 : 400,
              }}
              secondaryTypographyProps={{
                fontSize: '0.75rem',
                color:
                  location.pathname === item.path
                    ? theme.palette.primary.contrastText
                    : theme.palette.text.secondary,
              }}
            />
          </ListItemButton>
        </ListItem>
      ))}
    </>
  );

  const drawerContent = (
    <Box sx={{ overflow: 'auto', height: '100%' }}>
      {/* Sidebar header */}
      <Box
        sx={{
          p: 2,
          background: `linear-gradient(135deg, ${theme.palette.primary.main} 0%, ${theme.palette.primary.dark} 100%)`,
          color: theme.palette.primary.contrastText,
        }}
      >
        <Box sx={{ display: 'flex', alignItems: 'center', mb: 1 }}>
          <TrendingUpIcon sx={{ mr: 1 }} />
          <Typography variant='h6' sx={{ fontWeight: 600 }}>
            EconGraph
          </Typography>
        </Box>
        <Typography variant='body2' sx={{ opacity: 0.8 }}>
          Economic Data Visualization
        </Typography>
      </Box>

      {/* Navigation items */}
      <List sx={{ pt: 2 }}>
        {renderNavigationItems(navigationItems)}
        {renderNavigationItems(secondaryItems, true)}
      </List>

      {/* Footer info */}
      <Box
        sx={{
          position: 'absolute',
          bottom: 0,
          left: 0,
          right: 0,
          p: 2,
          borderTop: `1px solid ${theme.palette.divider}`,
          backgroundColor: theme.palette.background.paper,
        }}
      >
        <Typography variant='caption' color='text.secondary'>
          Built with modern web technologies
        </Typography>
      </Box>
    </Box>
  );

  return (
    <Drawer
      variant={isMobile ? 'temporary' : 'persistent'}
      open={open}
      onClose={onClose}
      ModalProps={{
        keepMounted: true, // Better mobile performance
      }}
      sx={{
        width: drawerWidth,
        flexShrink: 0,
        '& .MuiDrawer-paper': {
          width: drawerWidth,
          boxSizing: 'border-box',
          borderRight: `1px solid ${theme.palette.divider}`,
          background: theme.palette.background.default,
        },
      }}
      role="navigation"
      aria-label="Main navigation"
    >
      {drawerContent}
    </Drawer>
  );
};

export default Sidebar;
