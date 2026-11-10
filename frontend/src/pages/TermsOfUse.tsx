/**
 * Terms of Use page (ECO-881).
 *
 * Passes each public source's conditions on to the people who reuse its data, as the sources'
 * terms require: the citations and notices are the ones in
 * backend/crates/econ-graph-graphql/src/source_notices.rs, and each source's duties come from its
 * licence review (BLS ECO-769, BEA ECO-770, Census ECO-771, FHFA ECO-772, World Bank ECO-768,
 * FRED ECO-766). The public sources are the `catalog-public` role in
 * config/keycloak/econ-graph-realm.json; change PUBLIC_SOURCES when that role changes.
 *
 * DRAFT: this text needs the project owner's sign-off before release.
 */

import React from 'react';
import { Box, Container, Typography, Paper, Divider, List, ListItem, Link } from '@mui/material';
import { Gavel } from '@mui/icons-material';

const LAST_UPDATED = 'November 10, 2026';
const REPOSITORY_URL = 'https://github.com/EconGraph/econ-graph';
const WORLD_BANK_TERMS_URL = 'https://www.worldbank.org/ext/en/legal/terms-conditions/datasets';
const FRED_TERMS_URL = 'https://fred.stlouisfed.org/docs/api/terms_of_use.html';

interface PublicSource {
  name: string;
  /** The notice EconGraph shows with the source's data, quoted from its terms. */
  notice: string;
  /** What anyone who reuses the data must do. */
  duties: React.ReactNode[];
  termsUrl: string;
}

const PUBLIC_SOURCES: PublicSource[] = [
  {
    name: 'U.S. Bureau of Labor Statistics (BLS)',
    notice:
      'BLS.gov cannot vouch for the data or analyses derived from these data after the data have been retrieved from BLS.gov.',
    duties: [
      'Cite the U.S. Bureau of Labor Statistics as the source.',
      'Give the date the data were retrieved from BLS. The series page and its CSV show it.',
      'Include the notice above.',
      'Do not use the BLS logo.',
    ],
    termsUrl: 'https://www.bls.gov/developers/termsOfService.htm',
  },
  {
    name: 'U.S. Bureau of Economic Analysis (BEA)',
    notice:
      'This product uses the Bureau of Economic Analysis (BEA) Data API but is not endorsed or certified by BEA.',
    duties: [
      'Cite "Source: U.S. Bureau of Economic Analysis".',
      'Do not use the BEA name or logo to suggest that BEA endorses you or your work.',
    ],
    termsUrl: 'https://apps.bea.gov/API/_pdf/bea_api_tos.pdf',
  },
  {
    name: 'U.S. Census Bureau',
    notice:
      'This product uses the Census Bureau Data API but is not endorsed or certified by the Census Bureau.',
    duties: [
      'Cite the Census Bureau as the source of the original data only, not of anything you derived from it.',
      'Do not suggest that the Census Bureau endorses you or your work.',
      'Do not use the data to identify any person, household, business or other entity.',
    ],
    termsUrl: 'https://www.census.gov/data/developers/about/terms-of-service.html',
  },
  {
    name: 'Federal Housing Finance Agency (FHFA)',
    notice: 'This product uses FHFA Data but is neither endorsed nor certified by FHFA.',
    duties: [
      'Cite "Source: FHFA HPI®", with the index type where you know it.',
      'Keep the ® in "FHFA HPI®" and "FHFA House Price Index®". They are registered trademarks. Do not rename the indexes.',
      'Do not suggest that FHFA endorses you or your work.',
    ],
    termsUrl: 'https://www.fhfa.gov/about/fhfa-policies/api-terms-of-service',
  },
  {
    name: 'The World Bank (World Development Indicators)',
    notice:
      'World Bank data are licensed under CC BY 4.0. Anyone who reuses them must attribute The World Bank and its data providers in the format "The World Bank: Dataset name: Data source (if known)", as the World Bank\'s dataset terms of use require.',
    duties: [
      'Attribute the data as "The World Bank: Dataset name: Data source (if known)", for example "The World Bank: World Development Indicators".',
      'Say when you have changed the values, as CC BY 4.0 requires.',
      <>
        When you share the data, pass the same attribution requirement on to the people you share it
        with. You can do this by linking the{' '}
        <Link href={WORLD_BANK_TERMS_URL} target='_blank' rel='noopener noreferrer'>
          World Bank dataset terms of use
        </Link>
        .
      </>,
      'Do not use the World Bank name or logo, or suggest that the World Bank endorses you or your work.',
    ],
    termsUrl: WORLD_BANK_TERMS_URL,
  },
];

const bodyText = { color: 'text.secondary', marginBottom: '16px' } as const;

const TermsOfUse: React.FC = () => {
  return (
    <Container maxWidth='lg' sx={{ py: 4 }}>
      <Paper elevation={2} sx={{ p: 4, mb: 4, textAlign: 'center' }}>
        <Gavel sx={{ fontSize: 64, color: 'primary.main', mb: 2 }} />
        <Typography variant='h3' component='h1' gutterBottom>
          Terms of Use
        </Typography>
        <Typography variant='body1' sx={{ color: 'text.secondary' }}>
          Last updated: {LAST_UPDATED}
        </Typography>
      </Paper>

      <Paper elevation={1} sx={{ p: 3, mb: 4 }}>
        <Typography variant='h5' component='h2' gutterBottom>
          In short
        </Typography>
        <Typography variant='body1' sx={{ marginBottom: '16px' }}>
          EconGraph shows economic data published by statistical agencies. Each source&apos;s terms
          apply to its data. You may reuse data from the public sources below if you follow each
          source&apos;s conditions. EconGraph gives no warranty on the data.
        </Typography>
      </Paper>

      <Paper
        component='section'
        aria-labelledby='terms-public-sources'
        elevation={1}
        sx={{ p: 3, mb: 4 }}
      >
        <Typography id='terms-public-sources' variant='h5' component='h2' gutterBottom>
          Reusing data from the public sources
        </Typography>
        <Typography variant='body2' sx={bodyText}>
          These conditions apply wherever you reuse the data: on a page, in a chart, or in data you
          share from a CSV download, a JSON download or a GraphQL response. They summarise each
          source&apos;s terms. Where the two differ, the source&apos;s terms apply.
        </Typography>
        {PUBLIC_SOURCES.map(source => (
          <Box key={source.name} sx={{ mb: 3 }}>
            <Typography variant='h6' component='h3' gutterBottom>
              {source.name}
            </Typography>
            <Typography variant='body2' sx={{ mb: 1 }}>
              Notice: &ldquo;{source.notice}&rdquo;
            </Typography>
            <List dense sx={{ listStyleType: 'disc', pl: 3 }}>
              {source.duties.map((duty, index) => (
                <ListItem key={index} sx={{ display: 'list-item', py: 0 }}>
                  <Typography variant='body2'>{duty}</Typography>
                </ListItem>
              ))}
            </List>
            <Link href={source.termsUrl} target='_blank' rel='noopener noreferrer'>
              {source.name} terms of use
            </Link>
          </Box>
        ))}
      </Paper>

      <Paper
        component='section'
        aria-labelledby='terms-changed-values'
        elevation={1}
        sx={{ p: 3, mb: 4 }}
      >
        <Typography id='terms-changed-values' variant='h5' component='h2' gutterBottom>
          Values EconGraph computed
        </Typography>
        <Typography variant='body2' sx={bodyText}>
          Some values, such as percent changes, are computed by EconGraph from the source&apos;s
          data. EconGraph marks them as derived. They are not figures published by the source. Do
          not present them, or any values you changed, as the source&apos;s own figures.
        </Typography>
      </Paper>

      <Paper component='section' aria-labelledby='terms-fred' elevation={1} sx={{ p: 3, mb: 4 }}>
        <Typography id='terms-fred' variant='h5' component='h2' gutterBottom>
          FRED data
        </Typography>
        <Typography variant='body2' sx={bodyText}>
          Where EconGraph shows data from FRED®, Federal Reserve Bank of St. Louis: &ldquo;This
          product uses the FRED® API but is not endorsed or certified by the Federal Reserve Bank of
          St. Louis.&rdquo;
        </Typography>
        <Typography variant='body2' sx={bodyText}>
          If you use FRED data, you are bound by the{' '}
          <Link href={FRED_TERMS_URL} target='_blank' rel='noopener noreferrer'>
            FRED® API Terms of Use
          </Link>
          . They limit storing, republishing and commercial use of FRED data. EconGraph does not
          license FRED data to you.
        </Typography>
      </Paper>

      <Paper
        component='section'
        aria-labelledby='terms-restricted'
        elevation={1}
        sx={{ p: 3, mb: 4 }}
      >
        <Typography id='terms-restricted' variant='h5' component='h2' gutterBottom>
          Other sources
        </Typography>
        <Typography variant='body2' sx={bodyText}>
          Data from any source not listed under the public sources above is restricted. EconGraph
          does not license it for reuse, even if you can see it. That source&apos;s own terms decide
          what you may do with it.
        </Typography>
      </Paper>

      <Paper
        component='section'
        aria-labelledby='terms-acceptable-use'
        elevation={1}
        sx={{ p: 3, mb: 4 }}
      >
        <Typography id='terms-acceptable-use' variant='h5' component='h2' gutterBottom>
          Using the API and downloads
        </Typography>
        <Typography variant='body2' sx={bodyText}>
          The GraphQL API and the CSV and JSON downloads carry the same data, under the same source
          terms, as the pages. A series CSV includes its source&apos;s citation, notice and terms
          link. Keep them with the data when you share it. In GraphQL, each data source&apos;s
          notice is in its <code>notice</code> field.
        </Typography>
        <List dense sx={{ listStyleType: 'disc', pl: 3 }}>
          <ListItem sx={{ display: 'list-item', py: 0 }}>
            <Typography variant='body2'>
              Send requests at a reasonable rate. Do not degrade the service for other people.
            </Typography>
          </ListItem>
          <ListItem sx={{ display: 'list-item', py: 0 }}>
            <Typography variant='body2'>
              Do not try to get around sign-in or the access controls on restricted sources.
            </Typography>
          </ListItem>
          <ListItem sx={{ display: 'list-item', py: 0 }}>
            <Typography variant='body2'>
              Do not suggest that EconGraph or any source endorses you or your work.
            </Typography>
          </ListItem>
        </List>
        <Typography variant='body2' sx={bodyText}>
          We may block or limit clients that break these rules.
        </Typography>
      </Paper>

      <Paper
        component='section'
        aria-labelledby='terms-no-warranty'
        elevation={1}
        sx={{ p: 3, mb: 4 }}
      >
        <Typography id='terms-no-warranty' variant='h5' component='h2' gutterBottom>
          No warranty
        </Typography>
        <Typography variant='body2' sx={bodyText}>
          The data and the service are provided as they are. EconGraph copies the data from the
          sources and may transform it. It may be late, incomplete or wrong. We do not promise that
          the data are accurate, complete or current, or that they suit any purpose. Check figures
          against the source before you rely on them. The service may change or stop without notice.
        </Typography>
      </Paper>

      <Paper elevation={1} sx={{ p: 3, textAlign: 'center' }}>
        <Typography variant='body2' sx={{ color: 'text.secondary' }}>
          We may update these terms. The date above tells you when they last changed. To ask about
          them, open an issue in the project repository at{' '}
          <Link href={REPOSITORY_URL}>{REPOSITORY_URL}</Link>.
        </Typography>
        <Divider sx={{ my: 2 }} />
        <Typography variant='caption' sx={{ color: 'text.secondary' }}>
          EconGraph Terms of Use | Last updated: {LAST_UPDATED}
        </Typography>
      </Paper>
    </Container>
  );
};

export default TermsOfUse;
