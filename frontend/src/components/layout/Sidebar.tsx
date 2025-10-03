import React from 'react';
import { useNavigate, useLocation } from 'react-router-dom';
import { Drawer, Divider, Box, Typography, useTheme, useMediaQuery } from '@mui/material';
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
 * This provides intuitive navigation structure that improves on FRED's menu system.
 * @param root0 - The component props object.
 * @param root0.open - Whether the sidebar is open.
 * @param root0.onClose - Callback function to close the sidebar.
 * @returns JSX element representing the sidebar navigation.
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
      text: 'FRED Data',
      path: '/explore?source=Federal Reserve Economic Data',
      icon: <DataSourceIcon />,
      description: 'Federal Reserve Economic Data',
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
        <div
          key={item.path}
          data-testid={`sidebar-nav-${item.text.toLowerCase().replace(/\s+/g, '-')}`}
          role='button'
          tabIndex={0}
          aria-label={`Navigate to ${item.text}: ${item.description}`}
          aria-current={location.pathname === item.path ? 'page' : undefined}
          style={{
            display: 'flex',
            alignItems: 'center',
            minHeight: '48px',
            padding: '0 20px',
            cursor: 'pointer',
            backgroundColor:
              location.pathname === item.path ? theme.palette.primary.main : 'transparent',
            color:
              location.pathname === item.path
                ? theme.palette.primary.contrastText
                : theme.palette.text.primary,
            position: 'relative',
            left: '0',
            transform: 'translateX(0)',
            visibility: 'visible',
            width: '100%',
            boxSizing: 'border-box',
          }}
          onClick={() => handleNavigation(item.path)}
          onKeyDown={e => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault();
              handleNavigation(item.path);
            }
          }}
        >
          <div
            style={{
              minWidth: '0',
              marginRight: '24px',
              display: 'flex',
              justifyContent: 'center',
              alignItems: 'center',
            }}
          >
            {item.icon}
          </div>
          <div style={{ flex: 1 }}>
            <div
              data-testid={`sidebar-nav-${item.text.toLowerCase().replace(/\s+/g, '-')}-title`}
              style={{
                fontSize: '0.875rem',
                fontWeight: location.pathname === item.path ? 600 : 400,
                color: 'inherit',
              }}
            >
              {item.text}
            </div>
            <div
              data-testid={`sidebar-nav-${item.text.toLowerCase().replace(/\s+/g, '-')}-description`}
              style={{
                fontSize: '0.75rem',
                color:
                  location.pathname === item.path
                    ? theme.palette.primary.contrastText
                    : theme.palette.text.secondary,
                display: 'block',
              }}
            >
              {item.description}
            </div>
          </div>
        </div>
      ))}
    </>
  );

  const drawerContent = (
    <div
      data-testid='sidebar-content'
      style={{ overflow: 'auto', height: '100%', position: 'relative' }}
    >
      {/* Sidebar header - only show logo on mobile */}
      <Box
        data-testid='sidebar-header'
        sx={{
          p: 2,
          background: `linear-gradient(135deg, ${theme.palette.primary.main} 0%, ${theme.palette.primary.dark} 100%)`,
          color: theme.palette.primary.contrastText,
        }}
      >
        {isMobile && (
          <>
            <Box sx={{ display: 'flex', alignItems: 'center', mb: 1 }}>
              <TrendingUpIcon sx={{ mr: 1 }} />
              <Typography data-testid='sidebar-title' variant='h6' sx={{ fontWeight: 600 }}>
                EconGraph
              </Typography>
            </Box>
            <Typography data-testid='sidebar-subtitle' variant='body2' sx={{ opacity: 0.8 }}>
              Economic Data Visualization
            </Typography>
          </>
        )}
        {!isMobile && (
          <Typography data-testid='sidebar-subtitle' variant='body2' sx={{ opacity: 0.8 }}>
            Economic Data Visualization
          </Typography>
        )}
      </Box>

      {/* Navigation items */}
      <div data-testid='sidebar-navigation' style={{ paddingTop: '16px' }}>
        <div data-testid='sidebar-primary-nav'>{renderNavigationItems(navigationItems)}</div>
        <div data-testid='sidebar-secondary-nav'>{renderNavigationItems(secondaryItems, true)}</div>
      </div>

      {/* Footer info */}
      <Box
        data-testid='sidebar-footer'
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
        <Typography data-testid='sidebar-footer-text' variant='caption' color='text.secondary'>
          Built with modern web technologies
        </Typography>
      </Box>
    </div>
  );

  // For mobile, use Material-UI Drawer
  if (isMobile) {
    return (
      <Drawer
        variant='temporary'
        open={open}
        onClose={onClose}
        anchor='left'
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
        role='navigation'
        aria-label='Main navigation'
      >
        {drawerContent}
      </Drawer>
    );
  }

  // For desktop, use a simple Box instead of Drawer
  if (!open) return null;

  return (
    <div
      data-testid='sidebar-desktop'
      style={{
        width: `${drawerWidth}px`,
        flexShrink: 0,
        height: '100vh',
        position: 'fixed',
        top: '64px', // Below header
        left: '0px',
        zIndex: 9999,
        borderRight: '1px solid #ccc',
        background: '#f5f5f5',
        boxShadow: '2px 0 5px rgba(0,0,0,0.1)',
        overflow: 'auto',
        // Force positioning to ensure sidebar is visible
        transform: 'translateX(0)',
        visibility: 'visible',
      }}
      role='navigation'
      aria-label='Main navigation'
    >
      {drawerContent}
    </div>
  );
};

export default Sidebar;
