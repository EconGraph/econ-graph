import {
  buildSearchResultsCsv,
  buildSearchResultsJson,
  SEARCH_RESULTS_EXPORT_COLUMNS,
  type SearchResultsExportRow,
} from '../searchResultsExport';

const worldBank: SearchResultsExportRow = {
  source: 'World Bank',
  externalId: 'NY.GDP.PCAP.CD',
  title: 'GDP per capita (current US$), World',
  description: 'GDP per capita is gross domestic product divided by midyear population.',
  frequency: 'Annual',
  units: 'current US$',
  startDate: '1960-01-01',
  endDate: '2024-01-01',
  lastUpdated: '2026-10-01T00:00:00.000Z',
  id: '6d2f1c4e-0000-4000-8000-000000000001',
};

describe('search results export', () => {
  // ECO-417: the export carried only our id, so a row couldn't be matched back to the source.
  test('CSV has a fixed header naming the source and its series id', () => {
    const lines = buildSearchResultsCsv([worldBank]).split('\r\n');
    expect(lines[0]).toBe(
      'source,externalId,title,description,frequency,units,startDate,endDate,lastUpdated,id'
    );
    expect(lines[1]).toBe(
      'World Bank,NY.GDP.PCAP.CD,"GDP per capita (current US$), World",' +
        'GDP per capita is gross domestic product divided by midyear population.,' +
        'Annual,current US$,1960-01-01,2024-01-01,2026-10-01T00:00:00.000Z,' +
        '6d2f1c4e-0000-4000-8000-000000000001'
    );
    expect(lines[2]).toBe('');
  });

  test('CSV keeps its header with no rows and leaves missing fields empty', () => {
    expect(buildSearchResultsCsv([])).toBe(`${SEARCH_RESULTS_EXPORT_COLUMNS.join(',')}\r\n`);
    const [, row] = buildSearchResultsCsv([
      { source: 'FRED', externalId: 'GDP', title: 'Gross Domestic Product', id: 'x' },
    ]).split('\r\n');
    expect(row).toBe('FRED,GDP,Gross Domestic Product,,,,,,,x');
  });

  test('CSV text that could run as a formula gets a leading quote', () => {
    const [, row] = buildSearchResultsCsv([{ ...worldBank, title: '=cmd|/c calc' }]).split('\r\n');
    expect(row.split(',')[2]).toBe("'=cmd|/c calc");
  });

  test('JSON has every field, in CSV order, with null for missing ones', () => {
    const parsed = JSON.parse(
      buildSearchResultsJson([
        worldBank,
        { source: 'FRED', externalId: 'GDP', title: 'Gross Domestic Product', id: 'x' },
      ])
    );
    expect(parsed[0]).toEqual(worldBank);
    expect(Object.keys(parsed[0])).toEqual([...SEARCH_RESULTS_EXPORT_COLUMNS]);
    expect(parsed[1]).toEqual({
      source: 'FRED',
      externalId: 'GDP',
      title: 'Gross Domestic Product',
      description: null,
      frequency: null,
      units: null,
      startDate: null,
      endDate: null,
      lastUpdated: null,
      id: 'x',
    });
  });
});
