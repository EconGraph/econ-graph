import { DASHBOARD_SERIES, parseDashboardSeries } from '../dashboardSeries';
import realm from '../../../../config/keycloak/econ-graph-realm.json';

const valid = {
  id: 'gdp',
  label: 'Gross Domestic Product',
  sourceName: 'Bureau of Economic Analysis (BEA)',
  sourceLabel: 'BEA',
  externalId: 'bea_nipa/T10105.1.Q',
  fractionDigits: 2,
};

describe('parseDashboardSeries', () => {
  test('accepts well-formed entries', () => {
    expect(parseDashboardSeries([valid])).toEqual([valid]);
  });

  test('accepts a display scale with units for the scaled value', () => {
    const scaled = { ...valid, displayScale: 'trillions', units: 'Trillions of dollars' };
    expect(parseDashboardSeries([scaled])).toEqual([scaled]);
  });

  test('accepts units without a display scale', () => {
    const labelled = { ...valid, units: 'Percent' };
    expect(parseDashboardSeries([labelled])).toEqual([labelled]);
  });

  test('rejects a file that is not an array', () => {
    expect(() => parseDashboardSeries({})).toThrow('must be an array');
  });

  test.each(['id', 'label', 'sourceName', 'sourceLabel', 'externalId'])(
    'rejects an entry without %s',
    field => {
      expect(() => parseDashboardSeries([{ ...valid, [field]: '' }])).toThrow(`"${field}"`);
    }
  );

  test.each([undefined, -1, 1.5, 11, '2'])('rejects fractionDigits %j', fractionDigits => {
    expect(() => parseDashboardSeries([{ ...valid, fractionDigits }])).toThrow('fractionDigits');
  });

  test.each(['hundreds', 'Trillions', '', 12, null, 'toString'])(
    'rejects displayScale %j',
    displayScale => {
      expect(() => parseDashboardSeries([{ ...valid, displayScale, units: 'x' }])).toThrow(
        'displayScale'
      );
    }
  );

  test.each(['', '  ', 3])('rejects units %j', units => {
    expect(() => parseDashboardSeries([{ ...valid, units }])).toThrow('units');
  });

  test('rejects a display scale without units for the scaled value', () => {
    expect(() => parseDashboardSeries([{ ...valid, displayScale: 'trillions' }])).toThrow(
      'without "units"'
    );
  });

  test('rejects duplicate ids', () => {
    expect(() => parseDashboardSeries([valid, { ...valid }])).toThrow('more than once');
  });
});

describe('DASHBOARD_SERIES', () => {
  // The catalog read role for each source a card may use (econ-graph-auth's roles.rs).
  const READ_ROLE: Record<string, string> = {
    'Bureau of Economic Analysis (BEA)': 'catalog.bea:read',
    'Bureau of Labor Statistics (BLS)': 'catalog.bls:read',
  };

  test('reads only sources the realm grants the anonymous reader', () => {
    const anonymous = realm.clientScopeMappings['econ-graph-api'].find(
      mapping => mapping.client === 'econgraph-anonymous'
    );
    const publicRoles = realm.roles.realm.find(role => role.name === 'catalog-public')?.composites
      ?.client?.['econ-graph-api'];
    expect(anonymous).toBeDefined();
    expect(publicRoles).toBeDefined();
    for (const entry of DASHBOARD_SERIES) {
      const role = READ_ROLE[entry.sourceName];
      expect(role, `${entry.id}: ${entry.sourceName} has no public read role here`).toBeDefined();
      expect(anonymous?.roles).toContain(role);
      expect(publicRoles).toContain(role);
    }
  });
});
