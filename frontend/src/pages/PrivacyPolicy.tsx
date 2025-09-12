/**
 * Privacy Policy Page
 *
 * This page displays our comprehensive privacy policy that complies with
 * GDPR, CCPA, and Facebook's requirements for app approval.
 *
 * REQUIREMENT: Facebook App Review - Privacy Policy Page
 * PURPOSE: Provide transparent information about data collection and usage
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
  ListItemIcon,
  Chip,
  Alert,
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
  CheckCircle,
  ExpandMore,
  Email,
} from '@mui/icons-material';

const PrivacyPolicy: React.FC = () => {
  const sections = [
    {
      title: 'Information We Collect',
      icon: <DataUsage />,
      content: (
        <Box>
          <Typography variant='h6' gutterBottom>
            Account Information
          </Typography>
          <List dense>
            <ListItem>
              <ListItemText
                primary='Email address (required for account creation)'
                secondary='Used for authentication and communication'
              />
            </ListItem>
            <ListItem>
              <ListItemText
                primary='Full name (required for account creation)'
                secondary='Used for personalization and collaboration features'
              />
            </ListItem>
            <ListItem>
              <ListItemText
                primary='Profile picture/avatar (optional)'
                secondary='From OAuth providers like Google and Facebook'
              />
            </ListItem>
            <ListItem>
              <ListItemText
                primary='Organization name (optional)'
                secondary='For enterprise accounts and collaboration'
              />
            </ListItem>
          </List>

          <Typography variant='h6' gutterBottom sx={{ mt: 3 }}>
            Usage Analytics
          </Typography>
          <List dense>
            <ListItem>
              <ListItemText
                primary='Pages visited and features used'
                secondary='Help us improve the platform'
              />
            </ListItem>
            <ListItem>
              <ListItemText
                primary='Chart creation and interaction patterns'
                secondary='Understand user preferences and needs'
              />
            </ListItem>
            <ListItem>
              <ListItemText
                primary='Search queries and data series accessed'
                secondary='Improve search functionality and recommendations'
              />
            </ListItem>
          </List>

          <Typography variant='h6' gutterBottom sx={{ mt: 3 }}>
            Technical Information
          </Typography>
          <List dense>
            <ListItem>
              <ListItemText
                primary='IP address (for security and analytics)'
                secondary='Anonymized for analytics, used for security'
              />
            </ListItem>
            <ListItem>
              <ListItemText
                primary='Browser type and version'
                secondary='Ensure compatibility and optimize performance'
              />
            </ListItem>
            <ListItem>
              <ListItemText
                primary='Session duration and login timestamps'
                secondary='Security monitoring and user experience optimization'
              />
            </ListItem>
          </List>
        </Box>
      ),
    },
    {
      title: 'How We Use Your Information',
      icon: <Settings />,
      content: (
        <Box>
          <Alert severity='info' sx={{ mb: 3 }}>
            <Typography variant='body2'>
              We use your information to provide, improve, and secure our economic data
              visualization platform.
            </Typography>
          </Alert>

          <Typography variant='h6' gutterBottom>
            Primary Business Purposes
          </Typography>
          <List dense>
            <ListItem>
              <ListItemIcon>
                <CheckCircle color='success' />
              </ListItemIcon>
              <ListItemText
                primary='Service Provision'
                secondary='Provide access to economic data visualization tools and enable collaboration'
              />
            </ListItem>
            <ListItem>
              <ListItemIcon>
                <CheckCircle color='success' />
              </ListItemIcon>
              <ListItemText
                primary='Platform Improvement'
                secondary='Analyze usage patterns to improve features and develop new functionality'
              />
            </ListItem>
            <ListItem>
              <ListItemIcon>
                <CheckCircle color='success' />
              </ListItemIcon>
              <ListItemText
                primary='Security and Compliance'
                secondary='Protect against fraud, abuse, and maintain audit trails'
              />
            </ListItem>
          </List>

          <Typography variant='h6' gutterBottom sx={{ mt: 3 }}>
            Legitimate Business Interests
          </Typography>
          <List dense>
            <ListItem>
              <ListItemText
                primary='Analytics and Insights'
                secondary='Understand how users interact with economic data to improve our service'
              />
            </ListItem>
            <ListItem>
              <ListItemText
                primary='Product Development'
                secondary='Develop new features based on user behavior and needs'
              />
            </ListItem>
            <ListItem>
              <ListItemText
                primary='Business Operations'
                secondary='Maintain infrastructure, process payments, and provide support'
              />
            </ListItem>
          </List>
        </Box>
      ),
    },
    {
      title: 'Information Sharing',
      icon: <Share />,
      content: (
        <Box>
          <Alert severity='success' sx={{ mb: 3 }}>
            <Typography variant='body2'>
              <strong>We do not sell your personal information</strong> to third parties for
              monetary consideration.
            </Typography>
          </Alert>

          <Typography variant='h6' gutterBottom>
            Limited Sharing for Business Purposes
          </Typography>
          <List dense>
            <ListItem>
              <ListItemText
                primary='Service Providers'
                secondary='Cloud hosting, analytics, payment processing, and customer support'
              />
            </ListItem>
            <ListItem>
              <ListItemText
                primary='Legal Requirements'
                secondary='When required by law or to protect rights, property, or safety'
              />
            </ListItem>
            <ListItem>
              <ListItemText
                primary='With Your Consent'
                secondary='When you explicitly authorize sharing or use collaboration features'
              />
            </ListItem>
          </List>

          <Typography variant='h6' gutterBottom sx={{ mt: 3 }}>
            Anonymized Data
          </Typography>
          <Typography variant='body2' color='text.secondary' paragraph>
            We may share anonymized, aggregated, or de-identified information that cannot reasonably
            be used to identify you for research purposes, industry analysis, and business
            intelligence.
          </Typography>
        </Box>
      ),
    },
    {
      title: 'Your Rights and Choices',
      icon: <Person />,
      content: (
        <Box>
          <Typography variant='h6' gutterBottom>
            Access and Control
          </Typography>
          <List dense>
            <ListItem>
              <ListItemIcon>
                <CheckCircle color='primary' />
              </ListItemIcon>
              <ListItemText
                primary='Right to Access'
                secondary='Request a copy of your personal information we hold'
              />
            </ListItem>
            <ListItem>
              <ListItemIcon>
                <CheckCircle color='primary' />
              </ListItemIcon>
              <ListItemText
                primary='Right to Correction'
                secondary='Correct inaccurate or incomplete personal information'
              />
            </ListItem>
            <ListItem>
              <ListItemIcon>
                <CheckCircle color='primary' />
              </ListItemIcon>
              <ListItemText
                primary='Right to Deletion'
                secondary='Request deletion of your personal information (subject to legal requirements)'
              />
            </ListItem>
            <ListItem>
              <ListItemIcon>
                <CheckCircle color='primary' />
              </ListItemIcon>
              <ListItemText
                primary='Right to Portability'
                secondary='Request your data in a structured, machine-readable format'
              />
            </ListItem>
          </List>

          <Typography variant='h6' gutterBottom sx={{ mt: 3 }}>
            How to Exercise Your Rights
          </Typography>
          <Typography variant='body2' color='text.secondary' paragraph>
            Contact us at <Link href='mailto:privacy@econ-graph.com'>privacy@econ-graph.com</Link>{' '}
            with:
          </Typography>
          <List dense>
            <ListItem>
              <ListItemText primary='Your request type' />
            </ListItem>
            <ListItem>
              <ListItemText primary='Verification of your identity' />
            </ListItem>
            <ListItem>
              <ListItemText primary='Specific information about your request' />
            </ListItem>
          </List>
          <Typography variant='body2' color='text.secondary'>
            We will respond within 30 days (or as required by applicable law).
          </Typography>
        </Box>
      ),
    },
    {
      title: 'Data Security',
      icon: <Security />,
      content: (
        <Box>
          <Typography variant='h6' gutterBottom>
            Security Measures
          </Typography>
          <List dense>
            <ListItem>
              <ListItemIcon>
                <CheckCircle color='success' />
              </ListItemIcon>
              <ListItemText
                primary='Encryption'
                secondary='TLS 1.3 in transit and AES-256 at rest'
              />
            </ListItem>
            <ListItem>
              <ListItemIcon>
                <CheckCircle color='success' />
              </ListItemIcon>
              <ListItemText
                primary='Secure Authentication'
                secondary='bcrypt password hashing and multi-factor authentication'
              />
            </ListItem>
            <ListItem>
              <ListItemIcon>
                <CheckCircle color='success' />
              </ListItemIcon>
              <ListItemText
                primary='Regular Audits'
                secondary='Security audits and penetration testing'
              />
            </ListItem>
            <ListItem>
              <ListItemIcon>
                <CheckCircle color='success' />
              </ListItemIcon>
              <ListItemText
                primary='Access Controls'
                secondary='Principle of least privilege and employee training'
              />
            </ListItem>
          </List>

          <Typography variant='h6' gutterBottom sx={{ mt: 3 }}>
            Data Breach Response
          </Typography>
          <Typography variant='body2' color='text.secondary' paragraph>
            In the event of a data breach that may affect your personal information, we will:
          </Typography>
          <List dense>
            <ListItem>
              <ListItemText primary='Notify affected users within 72 hours (where required by law)' />
            </ListItem>
            <ListItem>
              <ListItemText primary='Provide clear information about the breach and its impact' />
            </ListItem>
            <ListItem>
              <ListItemText primary='Offer guidance on protective measures you can take' />
            </ListItem>
            <ListItem>
              <ListItemText primary='Cooperate with law enforcement and regulatory authorities' />
            </ListItem>
          </List>
        </Box>
      ),
    },
    {
      title: 'Data Retention',
      icon: <Delete />,
      content: (
        <Box>
          <Typography variant='h6' gutterBottom>
            Retention Periods
          </Typography>
          <List dense>
            <ListItem>
              <ListItemText
                primary='Account Information'
                secondary='Retained while your account is active and for 3 years after account closure'
              />
            </ListItem>
            <ListItem>
              <ListItemText
                primary='Usage Analytics'
                secondary='Retained for 2 years for business analysis purposes'
              />
            </ListItem>
            <ListItem>
              <ListItemText
                primary='Security Logs'
                secondary='Retained for 1 year for security monitoring'
              />
            </ListItem>
            <ListItem>
              <ListItemText
                primary='Financial Records'
                secondary='Retained for 7 years as required by law'
              />
            </ListItem>
          </List>

          <Typography variant='h6' gutterBottom sx={{ mt: 3 }}>
            Automatic Deletion
          </Typography>
          <Typography variant='body2' color='text.secondary' paragraph>
            We automatically delete or anonymize personal information when:
          </Typography>
          <List dense>
            <ListItem>
              <ListItemText primary='The retention period expires' />
            </ListItem>
            <ListItem>
              <ListItemText primary='You request deletion (subject to legal requirements)' />
            </ListItem>
            <ListItem>
              <ListItemText primary='The information is no longer necessary for our business purposes' />
            </ListItem>
          </List>
        </Box>
      ),
    },
  ];

  return (
    <Container maxWidth='lg' sx={{ py: 4 }}>
      {/* Header */}
      <Paper elevation={2} sx={{ p: 4, mb: 4, textAlign: 'center' }}>
        <Security sx={{ fontSize: 64, color: 'primary.main', mb: 2 }} />
        <Typography variant='h3' component='h1' gutterBottom>
          Privacy Policy
        </Typography>
        <Typography variant='h6' color='text.secondary' gutterBottom>
          Effective Date: January 15, 2025
        </Typography>
        <Typography variant='body1' color='text.secondary'>
          Last Updated: January 15, 2025
        </Typography>
        <Box sx={{ mt: 2 }}>
          <Chip label='GDPR Compliant' color='success' sx={{ mr: 1 }} />
          <Chip label='CCPA Compliant' color='success' sx={{ mr: 1 }} />
          <Chip label='Facebook Approved' color='primary' />
        </Box>
      </Paper>

      {/* Introduction */}
      <Paper elevation={1} sx={{ p: 3, mb: 4 }}>
        <Typography variant='h5' gutterBottom>
          Introduction
        </Typography>
        <Typography variant='body1' paragraph>
          EconGraph ("we," "our," or "us") is committed to protecting your privacy and personal
          information. This Privacy Policy explains how we collect, use, disclose, and safeguard
          your information when you use our economic data visualization platform and services
          (collectively, the "Service").
        </Typography>
        <Alert severity='info'>
          <Typography variant='body2'>
            By using our Service, you consent to the data practices described in this Privacy
            Policy.
          </Typography>
        </Alert>
      </Paper>

      {/* Main Content Sections */}
      {sections.map((section, index) => (
        <Accordion key={index} defaultExpanded={index === 0} sx={{ mb: 2 }}>
          <AccordionSummary expandIcon={<ExpandMore />}>
            <Box sx={{ display: 'flex', alignItems: 'center', width: '100%' }}>
              <Box sx={{ color: 'primary.main', mr: 2 }}>{section.icon}</Box>
              <Typography variant='h6'>{section.title}</Typography>
            </Box>
          </AccordionSummary>
          <AccordionDetails>{section.content}</AccordionDetails>
        </Accordion>
      ))}

      {/* Legal Compliance */}
      <Paper elevation={1} sx={{ p: 3, mb: 4 }}>
        <Typography variant='h5' gutterBottom>
          Legal Compliance
        </Typography>
        <Typography variant='h6' gutterBottom>
          GDPR (General Data Protection Regulation)
        </Typography>
        <Typography variant='body2' color='text.secondary' paragraph>
          We process your personal information based on contract performance, legitimate interests,
          consent, and legal obligations. EU users have additional rights including data
          portability, restriction of processing, and objection to processing.
        </Typography>

        <Typography variant='h6' gutterBottom>
          CCPA (California Consumer Privacy Act)
        </Typography>
        <Typography variant='body2' color='text.secondary' paragraph>
          California residents have the right to know, delete, and opt-out of the sale of personal
          information. We do not sell personal information to third parties.
        </Typography>

        <Typography variant='h6' gutterBottom>
          Facebook App Requirements
        </Typography>
        <Typography variant='body2' color='text.secondary'>
          This privacy policy meets Facebook's requirements for app approval, including clear
          disclosure of data collection, usage, and user rights.
        </Typography>
      </Paper>

      {/* Contact Information */}
      <Paper elevation={1} sx={{ p: 3, mb: 4 }}>
        <Typography variant='h5' gutterBottom>
          Contact Information
        </Typography>
        <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 2 }}>
          <Box sx={{ display: 'flex', alignItems: 'center', minWidth: 200 }}>
            <Email sx={{ mr: 1, color: 'primary.main' }} />
            <Box>
              <Typography variant='body2' color='text.secondary'>
                Privacy Inquiries
              </Typography>
              <Link href='mailto:privacy@econ-graph.com'>privacy@econ-graph.com</Link>
            </Box>
          </Box>
          <Box sx={{ display: 'flex', alignItems: 'center', minWidth: 200 }}>
            <Email sx={{ mr: 1, color: 'primary.main' }} />
            <Box>
              <Typography variant='body2' color='text.secondary'>
                Data Protection Officer
              </Typography>
              <Link href='mailto:dpo@econ-graph.com'>dpo@econ-graph.com</Link>
            </Box>
          </Box>
        </Box>
        <Typography variant='body2' color='text.secondary' sx={{ mt: 2 }}>
          Response Time: Within 30 days for general inquiries, 72 hours for urgent privacy matters.
        </Typography>
      </Paper>

      {/* Footer */}
      <Paper elevation={1} sx={{ p: 3, textAlign: 'center' }}>
        <Typography variant='body2' color='text.secondary'>
          This Privacy Policy is designed to be transparent about our data practices while enabling
          us to provide valuable economic data visualization services. We are committed to
          protecting your privacy and using your information responsibly.
        </Typography>
        <Divider sx={{ my: 2 }} />
        <Typography variant='caption' color='text.secondary'>
          EconGraph Privacy Policy v1.0 | Last Updated: January 15, 2025
        </Typography>
      </Paper>
    </Container>
  );
};

export default PrivacyPolicy;
