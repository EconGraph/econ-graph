/**
 * Privacy Policy page.
 *
 * Describes what EconGraph stores about a signed-in user:
 * an identity provider account signed in through Keycloak, the annotations and comments
 * a user writes, and the theme preference kept in the browser, plus ordinary
 * server logs. Nothing else about visitors is collected.
 *
 * DRAFT: this text needs the project owner's sign-off before release.
 */

import React from 'react';
import {
  Box,
  Container,
  Typography,
  Paper,
  Divider,
  List,
  ListItem,
  ListItemText,
  Accordion,
  AccordionSummary,
  AccordionDetails,
  Link,
} from '@mui/material';
import {
  Security,
  DataUsage,
  Person,
  Settings,
  Share,
  Delete,
  ExpandMore,
} from '@mui/icons-material';

const LAST_UPDATED = 'October 26, 2026';
const REPOSITORY_URL = 'https://github.com/EconGraph/econ-graph';

const PrivacyPolicy: React.FC = () => {
  const sections = [
    {
      title: 'What we store',
      icon: <DataUsage />,
      content: (
        <Box>
          <Typography
            variant='body1'
            sx={{
              marginBottom: '16px',
            }}
          >
            You can browse every chart and every public annotation on EconGraph without an account.
            We store information about you only when you sign in.
          </Typography>

          <Typography variant='h6' component='h3' gutterBottom>
            Your account
          </Typography>
          <Typography
            variant='body2'
            sx={{
              color: 'text.secondary',
              marginBottom: '16px',
            }}
          >
            Sign-in uses Keycloak with Google or Facebook when configured. The provider you choose
            shares your profile with Keycloak, and EconGraph keeps:
          </Typography>
          <List dense>
            <ListItem>
              <ListItemText
                primary='A stable account identifier'
                secondary='Issued by Keycloak and used as your user id in EconGraph'
              />
            </ListItem>
            <ListItem>
              <ListItemText
                primary='Email address'
                secondary='From your chosen sign-in provider, used to identify you'
              />
            </ListItem>
            <ListItem>
              <ListItemText
                primary='Name and profile picture, when supplied'
                secondary='From your chosen sign-in provider, shown next to your public annotations and comments'
              />
            </ListItem>
            <ListItem>
              <ListItemText
                primary='When you last signed in'
                secondary='Updated each time you sign in'
              />
            </ListItem>
          </List>
          <Typography
            variant='body2'
            sx={{
              color: 'text.secondary',
              marginBottom: '16px',
            }}
          >
            EconGraph never sees your sign-in provider's password, and we do not ask for anything
            beyond your basic profile.
          </Typography>

          <Typography variant='h6' component='h3' gutterBottom sx={{ mt: 3 }}>
            What you write
          </Typography>
          <List dense>
            <ListItem>
              <ListItemText
                primary='Annotations'
                secondary='The annotation, the series it belongs to, when you wrote it, and whether it is private or public'
              />
            </ListItem>
            <ListItem>
              <ListItemText
                primary='Comments'
                secondary='Comments you add to an annotation, and when you wrote them'
              />
            </ListItem>
          </List>

          <Typography variant='h6' component='h3' gutterBottom sx={{ mt: 3 }}>
            In your browser
          </Typography>
          <List dense>
            <ListItem>
              <ListItemText
                primary='Sign-in session'
                secondary='Keycloak sets a session cookie so you stay signed in. EconGraph keeps your access token in memory while the tab is open'
              />
            </ListItem>
            <ListItem>
              <ListItemText
                primary='Theme preference'
                secondary='Light or dark mode, kept in your browser'
              />
            </ListItem>
          </List>

          <Typography variant='h6' component='h3' gutterBottom sx={{ mt: 3 }}>
            Server logs
          </Typography>
          <Typography
            variant='body2'
            sx={{
              color: 'text.secondary',
              marginBottom: '16px',
            }}
          >
            Like any web server, ours records requests, including your IP address, for security and
            troubleshooting. We do not use analytics or advertising trackers.
          </Typography>
        </Box>
      ),
    },
    {
      title: 'How we use it',
      icon: <Settings />,
      content: (
        <Box>
          <List dense>
            <ListItem>
              <ListItemText
                primary='To run the service'
                secondary='Your account lets you create annotations and comments, and decide who sees them'
              />
            </ListItem>
            <ListItem>
              <ListItemText
                primary='To attribute what you write'
                secondary='Your name and picture appear next to your public annotations and comments'
              />
            </ListItem>
            <ListItem>
              <ListItemText
                primary='To keep the service secure'
                secondary='Server logs help us find abuse and fix problems'
              />
            </ListItem>
          </List>
          <Typography
            variant='body2'
            sx={{
              color: 'text.secondary',
              marginBottom: '16px',
            }}
          >
            We do not build profiles, send marketing email or use your data to train models.
          </Typography>
        </Box>
      ),
    },
    {
      title: 'Private and public annotations',
      icon: <Share />,
      content: (
        <Box>
          <Typography
            variant='body1'
            sx={{
              marginBottom: '16px',
            }}
          >
            Every annotation is either private or public. New annotations are private.
          </Typography>
          <List dense>
            <ListItem>
              <ListItemText
                primary='Private'
                secondary='Only you can see the annotation and its comments. The server enforces this; other users and anonymous visitors are never sent private annotations'
              />
            </ListItem>
            <ListItem>
              <ListItemText
                primary='Public'
                secondary='Anyone who opens the series, signed in or not, can read the annotation and its comments, together with your name'
              />
            </ListItem>
          </List>
          <Typography
            variant='body2'
            sx={{
              color: 'text.secondary',
              marginBottom: '16px',
            }}
          >
            You can switch an annotation between private and public at any time. A comment is
            visible exactly when the annotation it belongs to is visible.
          </Typography>
        </Box>
      ),
    },
    {
      title: 'Who else sees your data',
      icon: <Person />,
      content: (
        <Box>
          <Typography
            variant='body1'
            sx={{
              marginBottom: '16px',
            }}
          >
            We do not sell or share personal information. The parties involved in running the site
            are:
          </Typography>
          <List dense>
            <ListItem>
              <ListItemText
                primary='Your sign-in provider (Google or Facebook)'
                secondary='Handles your sign-in under its own privacy policy. The provider you choose learns that you signed in to EconGraph'
              />
            </ListItem>
            <ListItem>
              <ListItemText
                primary='Our hosting provider'
                secondary='Runs the servers and database that store the data described above'
              />
            </ListItem>
            <ListItem>
              <ListItemText
                primary='Google Fonts'
                secondary='The site loads its fonts and icons from Google Fonts, which sees your IP address like any web server'
              />
            </ListItem>
          </List>
          <Typography
            variant='body2'
            sx={{
              color: 'text.secondary',
              marginBottom: '16px',
            }}
          >
            The economic data shown on EconGraph comes from public statistical sources (FRED, BLS,
            Census BDS, FHFA, BEA, the World Bank and the Bank of Canada). Reading it sends nothing
            about you to those sources.
          </Typography>
        </Box>
      ),
    },
    {
      title: 'Deleting your data',
      icon: <Delete />,
      content: (
        <Box>
          <List dense>
            <ListItem>
              <ListItemText
                primary='Annotations and comments'
                secondary='You can edit or delete any annotation you wrote, at any time, together with its comments'
              />
            </ListItem>
            <ListItem>
              <ListItemText
                primary='Your account'
                secondary='Ask us (below) and we delete your account and everything you wrote'
              />
            </ListItem>
            <ListItem>
              <ListItemText
                primary='Server logs'
                secondary='Kept for a limited time for security, then discarded'
              />
            </ListItem>
          </List>
          <Typography
            variant='body2'
            sx={{
              color: 'text.secondary',
              marginBottom: '16px',
            }}
          >
            You can also revoke EconGraph's access from your Google account settings or from
            Facebook's Apps and Websites settings.
          </Typography>
        </Box>
      ),
    },
  ];

  return (
    <Container maxWidth='lg' sx={{ py: 4 }}>
      <Paper elevation={2} sx={{ p: 4, mb: 4, textAlign: 'center' }}>
        <Security sx={{ fontSize: 64, color: 'primary.main', mb: 2 }} />
        <Typography variant='h3' component='h1' gutterBottom>
          Privacy Policy
        </Typography>
        <Typography
          variant='body1'
          sx={{
            color: 'text.secondary',
          }}
        >
          Last updated: {LAST_UPDATED}
        </Typography>
      </Paper>

      <Paper elevation={1} sx={{ p: 3, mb: 4 }}>
        <Typography variant='h5' component='h2' gutterBottom>
          In short
        </Typography>
        <Typography
          variant='body1'
          sx={{
            marginBottom: '16px',
          }}
        >
          EconGraph is an open-source economic data explorer. Browsing needs no account. If you sign
          in through a configured provider, we keep your basic profile and the annotations and
          comments you write. Annotations are private unless you make them public. We use no
          trackers and share no personal information with third parties.
        </Typography>
      </Paper>

      {sections.map((section, index) => (
        <Accordion
          key={section.title}
          defaultExpanded={index === 0}
          sx={{ mb: 2 }}
          slotProps={{ heading: { component: 'h2' } }}
        >
          <AccordionSummary expandIcon={<ExpandMore />}>
            <Box component='span' sx={{ display: 'flex', alignItems: 'center', width: '100%' }}>
              <Box component='span' sx={{ display: 'flex', color: 'primary.main', mr: 2 }}>
                {section.icon}
              </Box>
              <Typography component='span' variant='h6'>
                {section.title}
              </Typography>
            </Box>
          </AccordionSummary>
          <AccordionDetails>{section.content}</AccordionDetails>
        </Accordion>
      ))}

      <Paper id='account-deletion' elevation={1} sx={{ p: 3, mb: 4 }}>
        <Typography variant='h5' component='h2' gutterBottom>
          Account deletion and other requests
        </Typography>
        <Typography
          variant='body2'
          sx={{
            color: 'text.secondary',
            marginBottom: '16px',
          }}
        >
          To ask about this policy, request a copy of your data or have your account deleted, open
          an issue in the project repository at <Link href={REPOSITORY_URL}>{REPOSITORY_URL}</Link>.
          For deletion, say that you want your EconGraph account and associated data removed. This
          applies whether you signed in with Google or Facebook. Do not post passwords, access
          tokens or other credentials. If your request involves personal details you would rather
          not post publicly, say so in the issue and we will follow up privately.
        </Typography>
      </Paper>

      <Paper elevation={1} sx={{ p: 3, textAlign: 'center' }}>
        <Typography
          variant='body2'
          sx={{
            color: 'text.secondary',
          }}
        >
          We will update this page when the service changes what it stores. The date above tells you
          when it last changed.
        </Typography>
        <Divider sx={{ my: 2 }} />
        <Typography
          variant='caption'
          sx={{
            color: 'text.secondary',
          }}
        >
          EconGraph Privacy Policy | Last updated: {LAST_UPDATED}
        </Typography>
      </Paper>
    </Container>
  );
};

export default PrivacyPolicy;
