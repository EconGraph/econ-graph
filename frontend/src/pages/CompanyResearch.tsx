import React, { useState } from 'react';
import { Link as RouterLink, useParams } from 'react-router-dom';
import {
  Alert,
  Box,
  Button,
  CircularProgress,
  FormControl,
  InputLabel,
  Link,
  MenuItem,
  Paper,
  Select,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  TextField,
  Typography,
} from '@mui/material';
import {
  useResearchCompanies,
  useResearchCompany,
  useCompanyStatement,
  useCompanyComparisons,
  useCompanyOtherFacts,
  useCompanyFactHistory,
  type CompanyStatementKind,
  type CompanyStatementLine,
  type CompanyRevisionInput,
} from '../graphql/companyResearch';
import CompanyLineChart from '../components/company/CompanyLineChart';
import CompanyNotes from '../components/financial/CompanyNotes';
import {
  companyFilingUrl,
  companyPeriodLabel,
  companyValuesCsv,
  type CompanyValue,
} from '../utils/companyResearch';
import { downloadCsv } from '../utils/seriesCsv';

interface PageCursor {
  scope: string;
  after: string | null;
  back: (string | null)[];
}
const emptyPage: PageCursor = { scope: '', after: null, back: [] };
const nextPage = (page: PageCursor, after: string | null): PageCursor => ({
  ...page,
  after,
  back: [...page.back, page.after],
});
const previousPage = (page: PageCursor): PageCursor => ({
  ...page,
  after: page.back[page.back.length - 1] ?? null,
  back: page.back.slice(0, -1),
});

const STATEMENTS: { value: CompanyStatementKind; label: string }[] = [
  { value: 'INCOME_STATEMENT', label: 'Income statement' },
  { value: 'BALANCE_SHEET', label: 'Balance sheet' },
  { value: 'CASH_FLOW', label: 'Cash flow' },
];
const periodKey = (line: CompanyStatementLine) => `${line.periodStart ?? ''}/${line.periodEnd}`;
const lineId = (line: CompanyStatementLine) =>
  `${line.key}/${line.unit}/${line.taxonomy}/${line.concept}/${line.derivation ?? ''}`;
const lineValue = (line: CompanyStatementLine): CompanyValue => ({
  ...line,
  accession: line.provenance.length === 1 ? line.provenance[0].accession : null,
  filed: line.provenance.length === 1 ? line.provenance[0].filed : null,
  sourceAccessions: [...new Set(line.provenance.map(fact => fact.accession))],
});
/** Compare like periods only: instants, quarters, annual and trailing-year calculations. */
function durationGroup(
  line: Pick<CompanyValue, 'periodStart' | 'periodEnd' | 'derivation'>
): string {
  if (!line.periodStart) return 'instant';
  if (line.derivation === 'trailing_twelve_months') return 'ttm';
  const days = (Date.parse(line.periodEnd) - Date.parse(line.periodStart)) / 86400000;
  if (days >= 70 && days <= 110) return 'quarter';
  if (days >= 330 && days <= 380) return 'annual';
  return `duration-${Math.round(days / 7)}`;
}

function CompanySearch() {
  const [search, setSearch] = useState('');
  const [submitted, setSubmitted] = useState('');
  const [after, setAfter] = useState<string | null>(null);
  const query = useResearchCompanies(submitted, after);
  return (
    <Stack spacing={2}>
      <Typography variant='h4' component='h1'>
        Company research
      </Typography>
      <Typography>Find an SEC-filing company by name, ticker or CIK.</Typography>
      <Box
        component='form'
        onSubmit={event => {
          event.preventDefault();
          setAfter(null);
          setSubmitted(search.trim());
        }}
        sx={{ display: 'flex', gap: 1 }}
      >
        <TextField
          label='Find company'
          value={search}
          onChange={event => setSearch(event.target.value)}
          fullWidth
        />
        <Button type='submit' variant='contained'>
          Search
        </Button>
      </Box>
      {query.isPending && <CircularProgress aria-label='Loading companies' />}
      {query.isError && (
        <Alert severity='error'>
          Company search failed. <Button onClick={() => query.refetch()}>Retry</Button>
        </Alert>
      )}
      {query.data && !query.data.nodes.length && (
        <Alert severity='info'>No matching companies found.</Alert>
      )}
      <Stack component='ul' spacing={1} sx={{ pl: 2 }}>
        {query.data?.nodes.map(company => (
          <li key={company.cik}>
            <Link component={RouterLink} to={`/companies/${company.cik}`}>
              {company.name}
            </Link>{' '}
            {company.ticker ? `(${company.ticker}) · ` : ''}CIK {company.cik}
          </li>
        ))}
      </Stack>
      {query.data?.hasNextPage && (
        <Button onClick={() => setAfter(query.data?.endCursor ?? null)}>Next companies</Button>
      )}
      {after && <Button onClick={() => setAfter(null)}>First companies</Button>}
    </Stack>
  );
}

function CompanyDetail({ cik }: { cik: string }) {
  const company = useResearchCompany(cik);
  const [kind, setKind] = useState<CompanyStatementKind>('INCOME_STATEMENT');
  const [revisionMode, setRevisionMode] = useState<CompanyRevisionInput['mode']>('LATEST');
  const [revisionDetail, setRevisionDetail] = useState('');
  const [period, setPeriod] = useState('');
  const [selected, setSelected] = useState('');
  const [noteTarget, setNoteTarget] = useState('company');
  const [otherPage, setOtherPage] = useState<PageCursor>(emptyPage);
  const [historyPage, setHistoryPage] = useState<PageCursor>(emptyPage);
  const validRevision =
    revisionMode === 'AS_OF'
      ? /^\d{4}-\d{2}-\d{2}$/.test(revisionDetail)
      : revisionMode === 'ACCESSION'
        ? /^\d{10}-\d{2}-\d{6}$/.test(revisionDetail)
        : true;
  const revision: CompanyRevisionInput = {
    mode: revisionMode,
    ...(revisionMode === 'AS_OF' ? { asOf: revisionDetail } : {}),
    ...(revisionMode === 'ACCESSION' ? { accession: revisionDetail } : {}),
  };
  const statement = useCompanyStatement(cik, kind, revision, validRevision && !!company.data);
  const mappedLines = statement.data?.lines ?? [];
  const periods = [
    ...new Set([
      ...(statement.data?.periods ?? []).map(item => `${item.start ?? ''}/${item.end}`),
      ...mappedLines.map(periodKey),
    ]),
  ].sort((a, b) => {
    const [aStart, aEnd] = a.split('/');
    const [bStart, bEnd] = b.split('/');
    return bEnd.localeCompare(aEnd) || aStart.localeCompare(bStart);
  });
  const mappedPeriods = periods.filter(key => mappedLines.some(line => periodKey(line) === key));
  const currentPeriod = periods.includes(period) ? period : (mappedPeriods[0] ?? periods[0] ?? '');
  const [periodStart, periodEnd] = currentPeriod.split('/');
  const otherScope = JSON.stringify([cik, revision, currentPeriod]);
  const activeOtherPage =
    otherPage.scope === otherScope ? otherPage : { ...emptyPage, scope: otherScope };
  const otherFacts = useCompanyOtherFacts(
    cik,
    revision,
    periodStart || null,
    periodEnd ?? '',
    activeOtherPage.after,
    validRevision && !!company.data && !!currentPeriod
  );
  const unclassified = otherFacts.data?.unclassifiedFacts ?? [];
  const lines = [...mappedLines, ...unclassified];
  const mappedShown = mappedLines.filter(line => periodKey(line) === currentPeriod);
  const shown = lines.filter(line => periodKey(line) === currentPeriod);
  const selectedLine = shown.find(line => lineId(line) === selected);
  const history = selectedLine
    ? lines.filter(
        line =>
          line.key === selectedLine.key &&
          line.unit === selectedLine.unit &&
          line.taxonomy === selectedLine.taxonomy &&
          line.concept === selectedLine.concept &&
          durationGroup(line) === durationGroup(selectedLine)
      )
    : [];
  const historyByPeriod = new Map<string, CompanyStatementLine>();
  for (const line of history) {
    const existing = historyByPeriod.get(periodKey(line));
    if (!existing || line.derivation === null) historyByPeriod.set(periodKey(line), line);
  }
  if (selectedLine) historyByPeriod.set(periodKey(selectedLine), selectedLine);
  const selectedIsOther = !!selectedLine && unclassified.includes(selectedLine);
  const historyScope = JSON.stringify([
    cik,
    revision,
    selectedLine?.taxonomy,
    selectedLine?.concept,
    selectedLine?.unit,
  ]);
  const activeHistoryPage =
    historyPage.scope === historyScope ? historyPage : { ...emptyPage, scope: historyScope };
  const sourceHistory = useCompanyFactHistory(
    cik,
    selectedLine,
    revision,
    activeHistoryPage.after,
    validRevision && selectedIsOther
  );
  const chartValues: CompanyValue[] = selectedIsOther
    ? (sourceHistory.data?.nodes ?? [])
        .map(fact => ({
          ...fact,
          label: fact.label ?? fact.concept,
          derivation: null,
          sourceAccessions: [fact.accession],
        }))
        .filter(fact => durationGroup(fact) === durationGroup(selectedLine!))
    : [...historyByPeriod.values()].map(lineValue);

  const noteTargets: { id: string; label: string; anchor?: string }[] = [
    { id: 'company', label: 'Company' },
    ...(selectedLine
      ? [
          {
            id: 'line',
            label: `Statement line / chart: ${selectedLine.label}, ${companyPeriodLabel(selectedLine)}`,
            anchor: selectedLine.subjectAnchor ?? undefined,
          },
        ]
      : []),
    ...(selectedLine?.provenance.map(fact => ({
      id: `fact/${fact.factId}`,
      label: `Reported fact: ${fact.concept}, ${fact.accession}`,
      anchor: fact.anchor ?? undefined,
    })) ?? []),
    ...[
      ...new Map(
        (selectedLine?.provenance ?? []).map(fact => [
          fact.accession,
          {
            id: `filing/${fact.accession}`,
            label: `Filing: ${fact.accession}`,
            anchor: fact.filingAnchor ?? undefined,
          },
        ])
      ).values(),
    ],
  ];
  const selectedNoteTarget = noteTargets.find(target => target.id === noteTarget);
  const noteTargetAvailable =
    !!selectedNoteTarget && (selectedNoteTarget.id === 'company' || !!selectedNoteTarget.anchor);
  const comparisons = useCompanyComparisons(
    cik,
    periodStart || null,
    periodEnd ?? '',
    revision,
    validRevision && !!currentPeriod
  );
  if (!/^\d{1,10}$/.test(cik)) return <Alert severity='error'>Invalid company CIK.</Alert>;
  if (company.isPending) return <CircularProgress aria-label='Loading company' />;
  if (company.isError)
    return (
      <Alert severity='error'>
        Company details could not be loaded.{' '}
        <Button onClick={() => company.refetch()}>Retry</Button>
      </Alert>
    );
  if (!company.data) return <Alert severity='info'>Company not found.</Alert>;
  const profile = company.data;
  return (
    <Stack spacing={3}>
      <Link component={RouterLink} to='/companies'>
        Find another company
      </Link>
      <Box>
        <Typography component='h1' variant='h4'>
          {profile.name}
        </Typography>
        <Typography>
          {profile.ticker && `${profile.ticker} · `}CIK {profile.cik}
        </Typography>
        {profile.sicCode && (
          <Typography>
            SIC {profile.sicCode}: {profile.sicDescription ?? 'Industry description unavailable'}
          </Typography>
        )}
        <Typography variant='body2'>Company metadata updated {profile.updatedAt}</Typography>
      </Box>
      <Alert severity='info'>
        Normalized statements use standard-taxonomy, whole-company SEC companyfacts.
        Company-specific concepts, segments and original filing layouts are available only in the
        source filings.
      </Alert>
      <Stack direction={{ xs: 'column', sm: 'row' }} spacing={2}>
        <FormControl sx={{ minWidth: 180 }}>
          <InputLabel id='statement-kind-label'>Statement</InputLabel>
          <Select
            labelId='statement-kind-label'
            label='Statement'
            value={kind}
            onChange={event => {
              setKind(event.target.value);
              setSelected('');
              setPeriod('');
            }}
          >
            {STATEMENTS.map(item => (
              <MenuItem value={item.value} key={item.value}>
                {item.label}
              </MenuItem>
            ))}
          </Select>
        </FormControl>
        <FormControl sx={{ minWidth: 180 }}>
          <InputLabel id='company-revision-label'>Revision</InputLabel>
          <Select
            labelId='company-revision-label'
            label='Revision'
            value={revisionMode}
            onChange={event => {
              setRevisionMode(event.target.value);
              setRevisionDetail('');
              setSelected('');
            }}
          >
            <MenuItem value='LATEST'>Latest reported</MenuItem>
            <MenuItem value='ORIGINAL'>Originally reported</MenuItem>
            <MenuItem value='AS_OF'>As filed by date</MenuItem>
            <MenuItem value='ACCESSION'>Specific filing</MenuItem>
          </Select>
        </FormControl>
        {(revisionMode === 'AS_OF' || revisionMode === 'ACCESSION') && (
          <TextField
            label={revisionMode === 'AS_OF' ? 'Filed by (YYYY-MM-DD)' : 'Accession number'}
            value={revisionDetail}
            onChange={event => setRevisionDetail(event.target.value)}
            helperText={
              validRevision ? ' ' : 'Enter a valid date or SEC accession to load this revision.'
            }
          />
        )}
      </Stack>
      {statement.isFetching && <CircularProgress aria-label='Loading statement' />}
      {statement.isError && (
        <Alert severity='error'>
          The statement could not be loaded.{' '}
          <Button onClick={() => statement.refetch()}>Retry</Button>
        </Alert>
      )}
      {statement.data?.unavailableDerivations?.map((item, index) => (
        <Alert severity='info' key={`${item.concept}/${item.periodEnd}/${index}`}>
          {item.concept} ({item.unit}), {item.periodStart ? `${item.periodStart} – ` : ''}
          {item.periodEnd}: {item.derivation} unavailable — {item.reason}
        </Alert>
      ))}
      {validRevision && statement.data && !periods.length && (
        <Alert severity='info'>No statement values are available for this revision.</Alert>
      )}
      {!!periods.length && (
        <FormControl sx={{ maxWidth: 400 }}>
          <InputLabel id='company-period-label'>Period</InputLabel>
          <Select
            labelId='company-period-label'
            label='Period'
            value={currentPeriod}
            onChange={event => {
              setPeriod(event.target.value);
              setSelected('');
            }}
          >
            {periods.map(key => {
              const [start, end] = key.split('/');
              const labelPeriod = { periodStart: start || null, periodEnd: end };
              return (
                <MenuItem key={key} value={key} data-statement-period={mappedPeriods.includes(key)}>
                  {companyPeriodLabel(labelPeriod)}
                </MenuItem>
              );
            })}
          </Select>
        </FormControl>
      )}
      {!!currentPeriod && (
        <>
          <Button
            disabled={!mappedShown.length}
            variant='outlined'
            onClick={() =>
              downloadCsv(
                `${profile.cik}-${kind.toLowerCase()}.csv`,
                companyValuesCsv(profile, mappedShown.map(lineValue), window.location.href)
              )
            }
          >
            Download statement CSV
          </Button>
          {!!currentPeriod && (
            <Box component='section' aria-label='Other facts pagination'>
              <Typography variant='body2'>
                Other reported facts: this page contains up to 100 facts for the selected period.
                Statement CSV includes mapped statement lines; other-facts CSV includes only this
                page.
              </Typography>
              {otherFacts.data && !unclassified.length && (
                <Typography>
                  No other reported facts on this page for the selected period.
                </Typography>
              )}
              {otherFacts.isPending && <CircularProgress aria-label='Loading other facts' />}
              {otherFacts.isError && (
                <Alert severity='error'>
                  Other reported facts could not be loaded.{' '}
                  <Button onClick={() => otherFacts.refetch()}>Retry</Button>
                </Alert>
              )}
              <Button
                disabled={!unclassified.length || otherFacts.isError || otherFacts.isPending}
                onClick={() =>
                  downloadCsv(
                    `${profile.cik}-other-facts-page.csv`,
                    companyValuesCsv(profile, unclassified.map(lineValue), window.location.href)
                  )
                }
              >
                Download other facts page CSV
              </Button>
              <Button
                disabled={!activeOtherPage.back.length}
                onClick={() => {
                  setOtherPage(previousPage(activeOtherPage));
                  setSelected('');
                }}
              >
                Previous other facts
              </Button>
              <Button
                disabled={
                  !otherFacts.data?.unclassifiedHasNextPage ||
                  !otherFacts.data.unclassifiedEndCursor
                }
                onClick={() => {
                  setOtherPage(
                    nextPage(activeOtherPage, otherFacts.data?.unclassifiedEndCursor ?? null)
                  );
                  setSelected('');
                }}
              >
                Next other facts
              </Button>
            </Box>
          )}
          {!mappedShown.length && statement.data && (
            <Alert severity='info'>
              No mapped statement lines are available for the selected period. Other reported facts
              remain available through the page controls.
            </Alert>
          )}
          <TableContainer component={Paper}>
            <Table aria-label={`${STATEMENTS.find(item => item.value === kind)?.label} values`}>
              <TableHead>
                <TableRow>
                  <TableCell>Statement line</TableCell>
                  <TableCell align='right'>Exact value</TableCell>
                  <TableCell>Unit</TableCell>
                  <TableCell>Source and revision</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {shown.map((line, index) => (
                  <React.Fragment key={lineId(line)}>
                    {unclassified.includes(line) &&
                      (index === 0 || !unclassified.includes(shown[index - 1])) && (
                        <TableRow>
                          <TableCell colSpan={4}>
                            <Typography component='h2' variant='h6'>
                              Other reported facts
                            </Typography>
                            <Typography variant='body2'>
                              These standard-taxonomy facts have no normalized statement
                              classification.
                            </Typography>
                          </TableCell>
                        </TableRow>
                      )}
                    <TableRow selected={line === selectedLine}>
                      <TableCell>
                        <Button
                          onClick={() => {
                            setSelected(lineId(line));
                            setNoteTarget('line');
                          }}
                          aria-label={`Chart ${line.label}`}
                        >
                          {line.label}
                        </Button>
                        <Typography
                          data-testid='company-concept'
                          variant='caption'
                          sx={{ display: 'block' }}
                        >
                          {line.taxonomy}:{line.concept}
                        </Typography>
                      </TableCell>
                      <TableCell align='right'>{line.value}</TableCell>
                      <TableCell>{line.unit}</TableCell>
                      <TableCell>
                        {line.restated && <Typography>Restated</Typography>}
                        {line.derivation && <Typography>{line.derivation}</Typography>}
                        {line.provenance.map(fact => {
                          const url = companyFilingUrl(cik, fact.accession);
                          return (
                            <Box key={`${fact.accession}/${fact.filed}`}>
                              {url ? (
                                <Link href={url} target='_blank' rel='noreferrer'>
                                  {fact.accession}
                                </Link>
                              ) : (
                                fact.accession
                              )}{' '}
                              · filed {fact.filed}
                            </Box>
                          );
                        })}
                      </TableCell>
                    </TableRow>
                  </React.Fragment>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
        </>
      )}
      {selectedLine && (
        <Box component='section' aria-label='Selected statement line'>
          <Typography variant='h5' component='h2'>
            {selectedLine.label} history
          </Typography>
          {selectedIsOther && (
            <Box>
              <Typography>
                Source history page: up to 100 facts. The chart and line CSV include only compatible
                periods on this history page.
              </Typography>
              {sourceHistory.data && (
                <Typography>
                  {chartValues.length} compatible values from {sourceHistory.data.nodes.length}{' '}
                  source facts on this page.
                </Typography>
              )}
              {sourceHistory.isPending && (
                <CircularProgress aria-label='Loading source fact history' />
              )}
              {sourceHistory.isError && (
                <Alert severity='error'>
                  Source fact history could not be loaded.{' '}
                  <Button onClick={() => sourceHistory.refetch()}>Retry</Button>
                </Alert>
              )}
              <Button
                disabled={!activeHistoryPage.back.length}
                onClick={() => setHistoryPage(previousPage(activeHistoryPage))}
              >
                Previous history page
              </Button>
              <Button
                disabled={!sourceHistory.data?.hasNextPage || !sourceHistory.data.endCursor}
                onClick={() =>
                  setHistoryPage(nextPage(activeHistoryPage, sourceHistory.data?.endCursor ?? null))
                }
              >
                Next history page
              </Button>
            </Box>
          )}
          {selectedIsOther && sourceHistory.data && !chartValues.length ? (
            <Alert severity='info'>
              No compatible economic periods are on this source-history page. Use the history page
              controls to browse the remaining source facts.
            </Alert>
          ) : (
            <CompanyLineChart values={chartValues} label={selectedLine.label} />
          )}
          <Button
            disabled={
              !chartValues.length ||
              (selectedIsOther && (sourceHistory.isPending || sourceHistory.isError))
            }
            onClick={() =>
              downloadCsv(
                `${profile.cik}-${selectedLine.concept}.csv`,
                companyValuesCsv(profile, chartValues, window.location.href)
              )
            }
          >
            Download line CSV
          </Button>
        </Box>
      )}
      {!!currentPeriod && (
        <Box component='section' aria-label='Ratios and peers'>
          <Typography component='h2' variant='h5'>
            Ratios and SIC peers
          </Typography>
          <Typography variant='body2'>
            Computed on request for matching actual periods; missing or incompatible inputs remain
            unavailable.
          </Typography>
          {comparisons.isPending && <CircularProgress aria-label='Loading ratios and peers' />}
          {comparisons.isError && (
            <Alert severity='error'>
              Ratios and peers could not be loaded.{' '}
              <Button onClick={() => comparisons.refetch()}>Retry</Button>
            </Alert>
          )}
          {comparisons.data && (
            <>
              <RatioTable ratios={comparisons.data.companyRatios} />
              {!comparisons.data.companyPeers.length && (
                <Typography>No SIC peers with comparable facts are available.</Typography>
              )}
              {comparisons.data.companyPeers.map(peer => (
                <Box key={peer.company.cik} sx={{ mt: 2 }}>
                  <Link component={RouterLink} to={`/companies/${peer.company.cik}`}>
                    {peer.company.name}
                  </Link>
                  <RatioTable ratios={peer.ratios} />
                </Box>
              ))}
            </>
          )}
        </Box>
      )}
      <Box component='section' aria-label='Research note target'>
        <FormControl fullWidth sx={{ mb: 1 }}>
          <InputLabel id='company-note-target-label'>Note target</InputLabel>
          <Select
            labelId='company-note-target-label'
            label='Note target'
            value={selectedNoteTarget ? noteTarget : ''}
            onChange={event => setNoteTarget(event.target.value)}
          >
            {noteTargets.map(target => (
              <MenuItem key={target.id} value={target.id}>
                {target.label}
              </MenuItem>
            ))}
          </Select>
        </FormControl>
        <Typography variant='body2'>
          Line and chart notes follow the economic subject. Reported-fact and filing notes stay
          attached to that specific filing.
        </Typography>
        {noteTargetAvailable ? (
          <CompanyNotes
            cik={profile.cik}
            anchor={selectedNoteTarget?.anchor}
            targetLabel={selectedNoteTarget?.label}
          />
        ) : (
          <Alert severity='info'>
            This note target is unavailable. Choose an available target to keep a note.
          </Alert>
        )}
      </Box>
      <Typography variant='body2'>
        Source:{' '}
        <Link
          href={`https://data.sec.gov/api/xbrl/companyfacts/CIK${profile.cik.padStart(10, '0')}.json`}
        >
          U.S. Securities and Exchange Commission (SEC), companyfacts
        </Link>
        . Values and filing dates reflect the selected revision.
      </Typography>
    </Stack>
  );
}

function RatioTable({
  ratios,
}: {
  ratios: import('../graphql/companyResearch').CompanyFinancialRatio[];
}) {
  return (
    <TableContainer>
      <Table size='small' aria-label='Financial ratios'>
        <TableHead>
          <TableRow>
            <TableCell>Ratio</TableCell>
            <TableCell>Value</TableCell>
            <TableCell>Inputs</TableCell>
          </TableRow>
        </TableHead>
        <TableBody>
          {ratios.map(ratio => (
            <TableRow key={ratio.name}>
              <TableCell>{ratio.name}</TableCell>
              <TableCell>
                {ratio.value === null
                  ? (ratio.unavailableReason ?? 'Unavailable')
                  : `${ratio.value} ${ratio.unit}`}
              </TableCell>
              <TableCell>
                {ratio.provenance.map((fact, index) => (
                  <Typography key={`${fact.accession}/${fact.concept}/${index}`} variant='body2'>
                    {fact.concept}: {fact.value} {fact.unit} ·{' '}
                    {fact.periodStart ? `${fact.periodStart} – ` : ''}
                    {fact.periodEnd} · {fact.accession}
                  </Typography>
                ))}
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </TableContainer>
  );
}

/** Gated company research uses real data-plane APIs and the existing app-owned notes. */
export default function CompanyResearch() {
  const { cik } = useParams<{ cik: string }>();
  return cik ? <CompanyDetail key={cik} cik={cik} /> : <CompanySearch />;
}
