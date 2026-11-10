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
export const FRED_TERMS_URL = 'https://fred.stlouisfed.org/docs/api/terms_of_use.html';
/** FRED's required notice, quoted from its API terms (source_notices.rs). */
export const FRED_NOTICE =
  'This product uses the FRED® API but is not endorsed or certified by the Federal Reserve Bank of St. Louis.';

export interface PublicSource {
  /** The source's catalog code, as in its `catalog.<code>:read` role. */
  catalog: string;
  name: string;
  /**
   * The notice EconGraph shows with the source's data (source_notices.rs). Most are quoted from
   * the source's terms; World Bank's is EconGraph's own wording of the attribution rule.
   */
  notice: string;
  /** Whether `notice` quotes the source's terms, and so is shown in quotation marks. */
  noticeIsQuote: boolean;
  /** What anyone who reuses the data must do, or what the source suggests. */
  duties: React.ReactNode[];
  termsUrl: string;
}

/**
 * The sources the `catalog-public` role makes public. The only list on the page: adding or
 * removing a source here is all the text needs, including for FRED (see "Other sources").
 */
export const PUBLIC_SOURCES: PublicSource[] = [
  {
    catalog: 'bls',
    name: 'U.S. Bureau of Labor Statistics (BLS)',
    notice:
      'BLS.gov cannot vouch for the data or analyses derived from these data after the data have been retrieved from BLS.gov.',
    noticeIsQuote: true,
    duties: [
      'Cite the U.S. Bureau of Labor Statistics as the source.',
      'Give the date the data were retrieved from BLS, where known. The series page and its CSV show it, or say that it was not recorded.',
      'Include the notice above.',
      'Do not use the BLS logo.',
    ],
    termsUrl: 'https://www.bls.gov/developers/termsOfService.htm',
  },
  {
    catalog: 'bea',
    name: 'U.S. Bureau of Economic Analysis (BEA)',
    notice:
      'This product uses the Bureau of Economic Analysis (BEA) Data API but is not endorsed or certified by BEA.',
    noticeIsQuote: true,
    duties: [
      'Credit BEA as the source. BEA suggests "Source: U.S. Bureau of Economic Analysis".',
      'Do not use the BEA name or logo to suggest that BEA endorses you or your work.',
    ],
    termsUrl: 'https://apps.bea.gov/API/_pdf/bea_api_tos.pdf',
  },
  {
    catalog: 'census',
    name: 'U.S. Census Bureau',
    notice:
      'This product uses the Census Bureau Data API but is not endorsed or certified by the Census Bureau.',
    noticeIsQuote: true,
    duties: [
      'Cite the Census Bureau as the source of the original data only, not of anything you derived from it.',
      'Do not suggest that the Census Bureau endorses you or your work.',
      'Do not use the data to identify any person, household, business or other entity.',
    ],
    termsUrl: 'https://www.census.gov/data/developers/about/terms-of-service.html',
  },
  {
    catalog: 'fhfa',
    name: 'Federal Housing Finance Agency (FHFA)',
    notice: 'This product uses FHFA Data but is neither endorsed nor certified by FHFA.',
    noticeIsQuote: true,
    duties: [
      'Credit FHFA as the source. FHFA suggests at least "Source: FHFA HPI®" or "Source: FHFA House Price Index®", ideally with the index type and seasonal adjustment.',
      '"FHFA HPI®" and "FHFA House Price Index®" are registered trademarks. Keep the ®, and do not rename the indexes.',
      'Do not suggest that FHFA endorses you or your work.',
    ],
    termsUrl: 'https://www.fhfa.gov/about/fhfa-policies/api-terms-of-service',
  },
  {
    catalog: 'world_bank',
    name: 'The World Bank (World Development Indicators)',
    notice:
      'World Bank data are licensed under CC BY 4.0. Anyone who reuses them must attribute The World Bank and its data providers in the format "The World Bank: Dataset name: Data source (if known)", as the World Bank\'s dataset terms of use require.',
    noticeIsQuote: false,
    duties: [
      'Attribute the data to The World Bank and its data providers in the format "The World Bank: Dataset name: Data source (if known)". For World Development Indicators that is "The World Bank: World Development Indicators:" followed by the indicator\'s data source. The World Bank gives the data source in its metadata for each indicator.',
      "If an indicator's World Bank metadata names a different licence, that licence applies, not CC BY 4.0.",
      'Say when you have changed the values, as CC BY 4.0 requires.',
      <>
        When you share the data, pass the same attribution requirement on to the people you share it
        with, and give the licence. You can do both by linking the{' '}
        <Link href={WORLD_BANK_TERMS_URL} target='_blank' rel='noopener noreferrer'>
          World Bank dataset terms of use
        </Link>
        , which state the CC BY 4.0 licence.
      </>,
      'Do not use the World Bank logo, or use its name to suggest that the World Bank sponsors or endorses you or your work. Using the name in the attribution above is required.',
    ],
    termsUrl: WORLD_BANK_TERMS_URL,
  },
];

const fredIsPublic = PUBLIC_SOURCES.some(source => source.catalog === 'fred');

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
          share from a download or a GraphQL response. They summarise each source&apos;s terms.
          Where the two differ, the source&apos;s terms apply.
        </Typography>
        {PUBLIC_SOURCES.map(source => (
          <Box key={source.name} sx={{ mb: 3 }}>
            <Typography variant='h6' component='h3' gutterBottom>
              {source.name}
            </Typography>
            <Typography variant='body2' sx={{ mb: 1 }}>
              EconGraph shows this notice with the data:{' '}
              {source.noticeIsQuote ? <>&ldquo;{source.notice}&rdquo;</> : source.notice}
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
          Where EconGraph shows FRED® data, it shows this notice: &ldquo;{FRED_NOTICE}&rdquo;
        </Typography>
        <Typography variant='body2' sx={bodyText}>
          If you use FRED data, you are bound by the{' '}
          <Link href={FRED_TERMS_URL} target='_blank' rel='noopener noreferrer'>
            FRED® API Terms of Use
          </Link>
          . Under those terms, you need the prior written permission of the Federal Reserve Bank of
          St. Louis to store or cache FRED data, add it to a database, republish or display it on a
          website, or use it commercially. Series that third parties own also need their
          owner&apos;s permission.
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
          {!fredIsPublic &&
            ' FRED is not one of the public sources, and EconGraph does not license FRED data for reuse.'}
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
          Data from the GraphQL API and the downloads is under the same source terms as the pages.
          The CSV download on a series page includes the source&apos;s citation, notice and terms
          link, and for BLS the retrieval date. Keep them with the data when you share it. The CSV
          and JSON downloads of search results hold series details only, not values or notices. In
          GraphQL, each data source&apos;s <code>notice</code> field is an object with its{' '}
          <code>citation</code>, <code>notice</code>, <code>termsUrl</code> and{' '}
          <code>retrievalDateRequired</code>.
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
          <Link href={REPOSITORY_URL} target='_blank' rel='noopener noreferrer'>
            {REPOSITORY_URL}
          </Link>
          .
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
