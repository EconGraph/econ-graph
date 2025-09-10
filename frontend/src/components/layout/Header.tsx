import React, { useState } from 'react';
import {
  AppBar,
  Toolbar,
  Typography,
  IconButton,
  Box,
  InputBase,
  alpha,
  styled,
  Button,
  Avatar,
  Menu,
  MenuItem,
  ListItemIcon,
  ListItemText,
  Divider,
} from '@mui/material';
import {
  Menu as MenuIcon,
  Search as SearchIcon,
  TrendingUp as TrendingUpIcon,
  Person as PersonIcon,
  Settings as SettingsIcon,
  ExitToApp as ExitToAppIcon,
  Analytics as AnalyticsIcon,
} from '@mui/icons-material';
import { useNavigate } from 'react-router-dom';
import { useAuth } from '../../contexts/AuthContext';
import LoginDialog from '../auth/LoginDialog';
import UserProfile from '../auth/UserProfile';

const Search = styled('div')(({ theme }) => ({
  position: 'relative',
  borderRadius: theme.shape.borderRadius,
  backgroundColor: alpha(theme.palette.common.white, 0.15),
  '&:hover': {
    backgroundColor: alpha(theme.palette.common.white, 0.25),
  },
  marginLeft: 0,
  width: '100%',
  [theme.breakpoints.up('sm')]: {
    marginLeft: theme.spacing(3),
    width: 'auto',
  },
}));

const SearchIconWrapper = styled('div')(({ theme }) => ({
  padding: theme.spacing(0, 2),
  height: '100%',
  position: 'absolute',
  pointerEvents: 'none',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
}));

const StyledInputBase = styled(InputBase)(({ theme }) => ({
  color: 'inherit',
  '& .MuiInputBase-input': {
    padding: theme.spacing(1, 1, 1, 0),
    paddingLeft: `calc(1em + ${theme.spacing(4)})`,
    transition: theme.transitions.create('width'),
    width: '100%',
    [theme.breakpoints.up('md')]: {
      width: '20ch',
    },
  },
}));

interface HeaderProps {
  onMenuClick: () => void;
}

/**
 * REQUIREMENT: Modern UI with intuitive navigation
 * PURPOSE: Application header with search functionality and responsive design
 * This provides quick access to search and navigation, improving on FRED's UX
 */
const Header: React.FC<HeaderProps> = ({ onMenuClick }) => {
  const { user, isAuthenticated, signOut } = useAuth();
  const navigate = useNavigate();
  const [searchQuery, setSearchQuery] = useState('');
  const [loginOpen, setLoginOpen] = useState(false);
  const [profileOpen, setProfileOpen] = useState(false);
  const [userMenuAnchor, setUserMenuAnchor] = useState<null | HTMLElement>(null);

  const handleSearchChange = (event: React.ChangeEvent<HTMLInputElement>) => {
    setSearchQuery(event.target.value);
  };

  const handleSearchSubmit = (event: React.FormEvent) => {
    event.preventDefault();
    if (searchQuery.trim()) {
      // REQUIREMENT: Search functionality similar to FRED
      // Navigate to search results page with the query
      navigate(`/explore?q=${encodeURIComponent(searchQuery.trim())}`);
    }
  };

  const handleLogoClick = () => {
    navigate('/');
  };

  const handleUserMenuOpen = (event: React.MouseEvent<HTMLElement>) => {
    setUserMenuAnchor(event.currentTarget);
  };

  const handleUserMenuClose = () => {
    setUserMenuAnchor(null);
  };

  const handleProfileClick = () => {
    setProfileOpen(true);
    handleUserMenuClose();
  };

  const handleSignOut = async () => {
    await signOut();
    handleUserMenuClose();
  };

  return (
    <>
      <AppBar
        position='fixed'
        sx={{
          zIndex: theme => theme.zIndex.drawer + 1,
          background: 'linear-gradient(90deg, #1976d2 0%, #1565c0 100%)',
        }}
      >
        <Toolbar component='nav' role='navigation'>
          {/* Menu button for mobile navigation */}
          <IconButton
            color='inherit'
            aria-label='open drawer'
            edge='start'
            onClick={onMenuClick}
            sx={{ mr: 2, display: { sm: 'none' } }}
          >
            <MenuIcon />
          </IconButton>

          {/* Application title and logo */}
          <Box
            sx={{
              display: 'flex',
              alignItems: 'center',
              flexGrow: 0,
              cursor: 'pointer',
              '&:hover': {
                opacity: 0.8,
              },
            }}
            onClick={handleLogoClick}
          >
            <TrendingUpIcon sx={{ mr: 1 }} />
            <Typography
              variant='h6'
              noWrap
              component='div'
              sx={{
                fontWeight: 600,
                letterSpacing: '0.5px',
              }}
            >
              EconGraph
            </Typography>
          </Box>

          {/* Search functionality */}
          <Box sx={{ flexGrow: 1, display: 'flex', justifyContent: 'center' }}>
            <Box
              component='form'
              onSubmit={handleSearchSubmit}
              sx={{ maxWidth: 600, width: '100%' }}
            >
              <Search sx={{ width: '100%' }}>
                <SearchIconWrapper>
                  <SearchIcon />
                </SearchIconWrapper>
                <StyledInputBase
                  placeholder='Search economic series...'
                  inputProps={{ 'aria-label': 'search economic series' }}
                  value={searchQuery}
                  onChange={handleSearchChange}
                />
              </Search>
            </Box>
          </Box>

          {/* Authentication and user menu */}
          <Box sx={{ flexGrow: 0, display: 'flex', alignItems: 'center', gap: 1 }}>
            {isAuthenticated ? (
              <>
                <Button
                  color='inherit'
                  startIcon={<AnalyticsIcon />}
                  onClick={() => navigate('/analysis')}
                  sx={{ display: { xs: 'none', md: 'flex' } }}
                >
                  Professional Analysis
                </Button>

                <IconButton onClick={handleUserMenuOpen} sx={{ p: 0 }}>
                  <Avatar src={user?.avatar} alt={user?.name} sx={{ width: 32, height: 32 }}>
                    {user?.name?.[0]}
                  </Avatar>
                </IconButton>

                <Menu
                  anchorEl={userMenuAnchor}
                  open={Boolean(userMenuAnchor)}
                  onClose={handleUserMenuClose}
                  transformOrigin={{ horizontal: 'right', vertical: 'top' }}
                  anchorOrigin={{ horizontal: 'right', vertical: 'bottom' }}
                >
                  <MenuItem disabled>
                    <Box>
                      <Typography variant='subtitle2'>{user?.name}</Typography>
                      <Typography variant='caption' color='text.secondary'>
                        {user?.email}
                      </Typography>
                    </Box>
                  </MenuItem>
                  <Divider />
                  <MenuItem onClick={handleProfileClick}>
                    <ListItemIcon>
                      <SettingsIcon fontSize='small' />
                    </ListItemIcon>
                    <ListItemText>Profile & Settings</ListItemText>
                  </MenuItem>
                  <MenuItem onClick={() => (window.location.href = '/analysis')}>
                    <ListItemIcon>
                      <AnalyticsIcon fontSize='small' />
                    </ListItemIcon>
                    <ListItemText>Professional Analysis</ListItemText>
                  </MenuItem>
                  <Divider />
                  <MenuItem onClick={handleSignOut}>
                    <ListItemIcon>
                      <ExitToAppIcon fontSize='small' />
                    </ListItemIcon>
                    <ListItemText>Sign Out</ListItemText>
                  </MenuItem>
                </Menu>
              </>
            ) : (
              <Button
                color='inherit'
                variant='outlined'
                startIcon={<PersonIcon />}
                onClick={() => setLoginOpen(true)}
                sx={{
                  borderColor: 'rgba(255, 255, 255, 0.5)',
                  '&:hover': {
                    borderColor: 'white',
                    backgroundColor: 'rgba(255, 255, 255, 0.1)',
                  },
                }}
              >
                Sign In
              </Button>
            )}
          </Box>
        </Toolbar>
      </AppBar>

      {/* Login Dialog */}
      <LoginDialog
        open={loginOpen}
        onClose={() => setLoginOpen(false)}
        onSuccess={() => setLoginOpen(false)}
      />

      {/* User Profile Dialog */}
      {user && <UserProfile open={profileOpen} onClose={() => setProfileOpen(false)} />}
    </>
  );
};

export default Header;
