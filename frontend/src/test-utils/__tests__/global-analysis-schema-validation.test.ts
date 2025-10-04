/**
 * Global Analysis GraphQL Schema Validation Tests.
 *
 * Test suite for validating Global Analysis GraphQL responses
 * against the backend schema to ensure consistency.
 */

import { loadGraphQLResponse } from '../mocks/graphql-response-loader';

describe('Global Analysis GraphQL Schema Validation', () => {
  describe('GetCountriesWithEconomicData', () => {
    it('should return valid countries with economic data structure', () => {
      const response = loadGraphQLResponse('get_countries_with_economic_data', 'success');
      
      expect(response).toBeDefined();
      expect(response.data).toBeDefined();
      expect(response.data.countriesWithEconomicData).toBeDefined();
      expect(Array.isArray(response.data.countriesWithEconomicData)).toBe(true);
      
      const countries = response.data.countriesWithEconomicData;
      expect(countries.length).toBeGreaterThan(0);
      
      // Validate first country structure
      const firstCountry = countries[0];
      expect(firstCountry.id).toBeDefined();
      expect(firstCountry.isoCode).toBeDefined();
      expect(firstCountry.isoCode2).toBeDefined();
      expect(firstCountry.name).toBeDefined();
      expect(firstCountry.region).toBeDefined();
      expect(firstCountry.subRegion).toBeDefined();
      expect(firstCountry.incomeGroup).toBeDefined();
      expect(firstCountry.population).toBeDefined();
      expect(firstCountry.gdpUsd).toBeDefined();
      expect(firstCountry.gdpPerCapitaUsd).toBeDefined();
      expect(firstCountry.latitude).toBeDefined();
      expect(firstCountry.longitude).toBeDefined();
      expect(firstCountry.currencyCode).toBeDefined();
      expect(firstCountry.isActive).toBeDefined();
      expect(firstCountry.economicIndicators).toBeDefined();
      expect(Array.isArray(firstCountry.economicIndicators)).toBe(true);
      
      // Validate economic indicators structure
      if (firstCountry.economicIndicators.length > 0) {
        const indicator = firstCountry.economicIndicators[0];
        expect(indicator.name).toBeDefined();
        expect(indicator.value).toBeDefined();
        expect(indicator.unit).toBeDefined();
        expect(indicator.year).toBeDefined();
        expect(indicator.source).toBeDefined();
        expect(indicator.description).toBeDefined();
        expect(indicator.category).toBeDefined();
        expect(indicator.frequency).toBeDefined();
      }
    });

    it('should handle error scenarios correctly', () => {
      const response = loadGraphQLResponse('get_countries_with_economic_data', 'error');
      
      expect(response).toBeDefined();
      expect(response.data).toBeNull();
      expect(response.errors).toBeDefined();
      expect(Array.isArray(response.errors)).toBe(true);
      expect(response.errors.length).toBeGreaterThan(0);
      
      const error = response.errors[0];
      expect(error.message).toBeDefined();
      expect(error.extensions).toBeDefined();
      expect(error.extensions.code).toBeDefined();
    });

    it('should handle loading states correctly', () => {
      const response = loadGraphQLResponse('get_countries_with_economic_data', 'loading');
      
      expect(response).toBeDefined();
      expect(response.data).toBeDefined();
      expect(response.data.countriesWithEconomicData).toBeDefined();
      expect(Array.isArray(response.data.countriesWithEconomicData)).toBe(true);
      expect(response.data.countriesWithEconomicData.length).toBe(0);
    });
  });

  describe('GetCorrelationNetwork', () => {
    it('should return valid correlation network structure', () => {
      const response = loadGraphQLResponse('get_correlation_network', 'success');
      
      expect(response).toBeDefined();
      expect(response.data).toBeDefined();
      expect(response.data.correlationNetwork).toBeDefined();
      expect(Array.isArray(response.data.correlationNetwork)).toBe(true);
      
      const network = response.data.correlationNetwork;
      expect(network.length).toBeGreaterThan(0);
      
      // Validate first network node structure
      const firstNode = network[0];
      expect(firstNode.id).toBeDefined();
      expect(firstNode.countryId).toBeDefined();
      expect(firstNode.countryName).toBeDefined();
      expect(firstNode.isoCode).toBeDefined();
      expect(firstNode.isoCode2).toBeDefined();
      expect(firstNode.region).toBeDefined();
      expect(firstNode.subRegion).toBeDefined();
      expect(firstNode.economicStrength).toBeDefined();
      expect(firstNode.centrality).toBeDefined();
      expect(firstNode.connections).toBeDefined();
      expect(Array.isArray(firstNode.connections)).toBe(true);
      
      // Validate connections structure
      if (firstNode.connections.length > 0) {
        const connection = firstNode.connections[0];
        expect(connection.targetCountryId).toBeDefined();
        expect(connection.targetCountryName).toBeDefined();
        expect(connection.correlation).toBeDefined();
        expect(connection.connectionType).toBeDefined();
        expect(connection.strength).toBeDefined();
      }
    });

    it('should handle error scenarios correctly', () => {
      const response = loadGraphQLResponse('get_correlation_network', 'error');
      
      expect(response).toBeDefined();
      expect(response.data).toBeNull();
      expect(response.errors).toBeDefined();
      expect(Array.isArray(response.errors)).toBe(true);
      expect(response.errors.length).toBeGreaterThan(0);
      
      const error = response.errors[0];
      expect(error.message).toBeDefined();
      expect(error.extensions).toBeDefined();
      expect(error.extensions.code).toBeDefined();
    });
  });

  describe('GetGlobalEventsWithImpacts', () => {
    it('should return valid global events structure', () => {
      const response = loadGraphQLResponse('get_global_events_with_impacts', 'success');
      
      expect(response).toBeDefined();
      expect(response.data).toBeDefined();
      expect(response.data.globalEventsWithImpacts).toBeDefined();
      expect(Array.isArray(response.data.globalEventsWithImpacts)).toBe(true);
      
      const events = response.data.globalEventsWithImpacts;
      expect(events.length).toBeGreaterThan(0);
      
      // Validate first event structure
      const firstEvent = events[0];
      expect(firstEvent.id).toBeDefined();
      expect(firstEvent.title).toBeDefined();
      expect(firstEvent.description).toBeDefined();
      expect(firstEvent.eventDate).toBeDefined();
      expect(firstEvent.eventType).toBeDefined();
      expect(firstEvent.severity).toBeDefined();
      expect(firstEvent.source).toBeDefined();
      expect(firstEvent.url).toBeDefined();
      expect(firstEvent.tags).toBeDefined();
      expect(Array.isArray(firstEvent.tags)).toBe(true);
      expect(firstEvent.countryImpacts).toBeDefined();
      expect(Array.isArray(firstEvent.countryImpacts)).toBe(true);
      
      // Validate country impacts structure
      if (firstEvent.countryImpacts.length > 0) {
        const impact = firstEvent.countryImpacts[0];
        expect(impact.countryId).toBeDefined();
        expect(impact.countryName).toBeDefined();
        expect(impact.isoCode).toBeDefined();
        expect(impact.isoCode2).toBeDefined();
        expect(impact.impactSeverity).toBeDefined();
        expect(impact.economicImpact).toBeDefined();
        expect(impact.recoveryTimeMonths).toBeDefined();
        expect(impact.confidence).toBeDefined();
      }
    });

    it('should handle error scenarios correctly', () => {
      const response = loadGraphQLResponse('get_global_events_with_impacts', 'error');
      
      expect(response).toBeDefined();
      expect(response.data).toBeNull();
      expect(response.errors).toBeDefined();
      expect(Array.isArray(response.errors)).toBe(true);
      expect(response.errors.length).toBeGreaterThan(0);
      
      const error = response.errors[0];
      expect(error.message).toBeDefined();
      expect(error.extensions).toBeDefined();
      expect(error.extensions.code).toBeDefined();
    });
  });

  describe('Data Type Validation', () => {
    it('should have correct data types for countries', () => {
      const response = loadGraphQLResponse('get_countries_with_economic_data', 'success');
      const countries = response.data.countriesWithEconomicData;
      
      countries.forEach((country: any) => {
        expect(typeof country.id).toBe('string');
        expect(typeof country.isoCode).toBe('string');
        expect(typeof country.isoCode2).toBe('string');
        expect(typeof country.name).toBe('string');
        expect(typeof country.region).toBe('string');
        expect(typeof country.isActive).toBe('boolean');
        
        if (country.population !== null) {
          expect(typeof country.population).toBe('number');
        }
        if (country.gdpUsd !== null) {
          expect(typeof country.gdpUsd).toBe('string');
        }
        if (country.latitude !== null) {
          expect(typeof country.latitude).toBe('string');
        }
        if (country.longitude !== null) {
          expect(typeof country.longitude).toBe('string');
        }
      });
    });

    it('should have correct data types for correlation network', () => {
      const response = loadGraphQLResponse('get_correlation_network', 'success');
      const network = response.data.correlationNetwork;
      
      network.forEach((node: any) => {
        expect(typeof node.id).toBe('string');
        expect(typeof node.countryId).toBe('string');
        expect(typeof node.countryName).toBe('string');
        expect(typeof node.isoCode).toBe('string');
        expect(typeof node.isoCode2).toBe('string');
        expect(typeof node.region).toBe('string');
        expect(typeof node.economicStrength).toBe('number');
        expect(typeof node.centrality).toBe('number');
        
        node.connections.forEach((connection: any) => {
          expect(typeof connection.targetCountryId).toBe('string');
          expect(typeof connection.targetCountryName).toBe('string');
          expect(typeof connection.correlation).toBe('number');
          expect(typeof connection.connectionType).toBe('string');
          expect(typeof connection.strength).toBe('number');
        });
      });
    });

    it('should have correct data types for global events', () => {
      const response = loadGraphQLResponse('get_global_events_with_impacts', 'success');
      const events = response.data.globalEventsWithImpacts;
      
      events.forEach((event: any) => {
        expect(typeof event.id).toBe('string');
        expect(typeof event.title).toBe('string');
        expect(typeof event.description).toBe('string');
        expect(typeof event.eventDate).toBe('string');
        expect(typeof event.eventType).toBe('string');
        expect(typeof event.severity).toBe('number');
        expect(typeof event.source).toBe('string');
        expect(typeof event.url).toBe('string');
        expect(Array.isArray(event.tags)).toBe(true);
        
        event.countryImpacts.forEach((impact: any) => {
          expect(typeof impact.countryId).toBe('string');
          expect(typeof impact.countryName).toBe('string');
          expect(typeof impact.isoCode).toBe('string');
          expect(typeof impact.isoCode2).toBe('string');
          expect(typeof impact.impactSeverity).toBe('number');
          expect(typeof impact.economicImpact).toBe('string');
          expect(typeof impact.recoveryTimeMonths).toBe('number');
          expect(typeof impact.confidence).toBe('number');
        });
      });
    });
  });

  describe('Edge Cases', () => {
    it('should handle empty arrays gracefully', () => {
      const response = loadGraphQLResponse('get_countries_with_economic_data', 'loading');
      
      expect(response.data.countriesWithEconomicData).toEqual([]);
    });

    it('should handle missing optional fields', () => {
      const response = loadGraphQLResponse('get_countries_with_economic_data', 'success');
      const countries = response.data.countriesWithEconomicData;
      
      countries.forEach((country: any) => {
        // Optional fields should be null or undefined, not cause errors
        expect(country.subRegion === null || typeof country.subRegion === 'string').toBe(true);
        expect(country.incomeGroup === null || typeof country.incomeGroup === 'string').toBe(true);
        expect(country.population === null || typeof country.population === 'number').toBe(true);
        expect(country.gdpUsd === null || typeof country.gdpUsd === 'string').toBe(true);
        expect(country.latitude === null || typeof country.latitude === 'string').toBe(true);
        expect(country.longitude === null || typeof country.longitude === 'string').toBe(true);
        expect(country.currencyCode === null || typeof country.currencyCode === 'string').toBe(true);
      });
    });
  });

  describe('Schema Consistency', () => {
    it('should maintain consistent field naming across all responses', () => {
      const countriesResponse = loadGraphQLResponse('get_countries_with_economic_data', 'success');
      const networkResponse = loadGraphQLResponse('get_correlation_network', 'success');
      const eventsResponse = loadGraphQLResponse('get_global_events_with_impacts', 'success');
      
      // All responses should follow camelCase naming convention
      expect(countriesResponse.data.countriesWithEconomicData).toBeDefined();
      expect(networkResponse.data.correlationNetwork).toBeDefined();
      expect(eventsResponse.data.globalEventsWithImpacts).toBeDefined();
    });

    it('should have consistent error structure', () => {
      const countriesError = loadGraphQLResponse('get_countries_with_economic_data', 'error');
      const networkError = loadGraphQLResponse('get_correlation_network', 'error');
      const eventsError = loadGraphQLResponse('get_global_events_with_impacts', 'error');
      
      [countriesError, networkError, eventsError].forEach(response => {
        expect(response.data).toBeNull();
        expect(response.errors).toBeDefined();
        expect(Array.isArray(response.errors)).toBe(true);
        expect(response.errors.length).toBeGreaterThan(0);
        
        const error = response.errors[0];
        expect(error.message).toBeDefined();
        expect(error.extensions).toBeDefined();
        expect(error.extensions.code).toBeDefined();
      });
    });
  });
});
