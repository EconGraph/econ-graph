# Global Analysis API Documentation

## Overview

This document describes the API endpoints, data structures, and integration patterns for the Global Analysis feature. The API provides access to country data, economic indicators, and analysis tools through RESTful endpoints and GraphQL queries.

## Table of Contents

- [API Endpoints](#api-endpoints)
- [Data Models](#data-models)
- [GraphQL Schema](#graphql-schema)
- [Authentication](#authentication)
- [Rate Limiting](#rate-limiting)
- [Error Handling](#error-handling)
- [Examples](#examples)
- [SDK Integration](#sdk-integration)

## API Endpoints

### Base URL
```
Production: https://api.econ-graph.com/v1
Staging: https://staging-api.econ-graph.com/v1
Development: http://localhost:9876/api/v1
```

### Country Data Endpoints

#### Get All Countries
```http
GET /countries
```

**Query Parameters:**
- `limit` (optional): Number of countries to return (default: 100, max: 1000)
- `offset` (optional): Number of countries to skip (default: 0)
- `region` (optional): Filter by region (e.g., "Europe", "Asia")
- `income_group` (optional): Filter by income group ("high", "upper_middle", "lower_middle", "low")
- `has_data` (optional): Filter countries with data for specific indicator (boolean)

**Response:**
```json
{
  "data": [
    {
      "id": "USA",
      "name": "United States",
      "region": "Americas",
      "subregion": "North America",
      "income_group": "high",
      "population": 331002651,
      "area": 9833517,
      "coordinates": {
        "latitude": 39.8283,
        "longitude": -98.5795
      },
      "economic_indicators": [
        {
          "name": "gdp",
          "value": 26949.95,
          "unit": "billion_usd",
          "date": "2023-12-31",
          "source": "World Bank"
        }
      ]
    }
  ],
  "pagination": {
    "total": 195,
    "limit": 100,
    "offset": 0,
    "has_next": true
  }
}
```

#### Get Country by ID
```http
GET /countries/{country_id}
```

**Path Parameters:**
- `country_id`: ISO 3166-1 alpha-3 country code

**Response:**
```json
{
  "data": {
    "id": "USA",
    "name": "United States",
    "region": "Americas",
    "subregion": "North America",
    "income_group": "high",
    "population": 331002651,
    "area": 9833517,
    "coordinates": {
      "latitude": 39.8283,
      "longitude": -98.5795
    },
    "economic_indicators": [
      {
        "name": "gdp",
        "value": 26949.95,
        "unit": "billion_usd",
        "date": "2023-12-31",
        "source": "World Bank"
      },
      {
        "name": "inflation",
        "value": 3.1,
        "unit": "percent",
        "date": "2023-12-31",
        "source": "IMF"
      }
    ],
    "metadata": {
      "last_updated": "2024-01-15T10:30:00Z",
      "data_quality": "high",
      "completeness": 0.95
    }
  }
}
```

### Economic Indicators Endpoints

#### Get Available Indicators
```http
GET /indicators
```

**Response:**
```json
{
  "data": [
    {
      "name": "gdp",
      "display_name": "Gross Domestic Product",
      "description": "Total economic output in billions USD",
      "unit": "billion_usd",
      "category": "economic",
      "frequency": "annual",
      "sources": ["World Bank", "IMF", "OECD"]
    },
    {
      "name": "inflation",
      "display_name": "Inflation Rate",
      "description": "Annual percentage change in consumer prices",
      "unit": "percent",
      "category": "monetary",
      "frequency": "monthly",
      "sources": ["IMF", "Central Banks"]
    }
  ]
}
```

#### Get Indicator Data
```http
GET /indicators/{indicator_name}/data
```

**Path Parameters:**
- `indicator_name`: Name of the economic indicator

**Query Parameters:**
- `countries` (optional): Comma-separated list of country IDs
- `start_date` (optional): Start date in YYYY-MM-DD format
- `end_date` (optional): End date in YYYY-MM-DD format
- `frequency` (optional): Data frequency ("daily", "monthly", "quarterly", "annual")

**Response:**
```json
{
  "indicator": {
    "name": "gdp",
    "display_name": "Gross Domestic Product",
    "unit": "billion_usd"
  },
  "data": [
    {
      "country_id": "USA",
      "country_name": "United States",
      "values": [
        {
          "date": "2023-12-31",
          "value": 26949.95,
          "source": "World Bank"
        },
        {
          "date": "2022-12-31",
          "value": 25462.70,
          "source": "World Bank"
        }
      ]
    }
  ],
  "metadata": {
    "last_updated": "2024-01-15T10:30:00Z",
    "total_countries": 195,
    "data_range": {
      "start_date": "2020-01-01",
      "end_date": "2023-12-31"
    }
  }
}
```

### Analysis Endpoints

#### Calculate Correlations
```http
POST /analysis/correlations
```

**Request Body:**
```json
{
  "countries": ["USA", "CHN", "DEU", "JPN"],
  "indicators": ["gdp", "inflation", "unemployment"],
  "start_date": "2020-01-01",
  "end_date": "2023-12-31",
  "method": "pearson"
}
```

**Response:**
```json
{
  "data": {
    "correlation_matrix": {
      "gdp": {
        "inflation": -0.23,
        "unemployment": -0.45
      },
      "inflation": {
        "unemployment": 0.67
      }
    },
    "statistical_significance": {
      "gdp_inflation": 0.05,
      "gdp_unemployment": 0.01,
      "inflation_unemployment": 0.02
    },
    "sample_size": 48
  }
}
```

#### Generate Network Analysis
```http
POST /analysis/network
```

**Request Body:**
```json
{
  "countries": ["USA", "CHN", "DEU", "JPN", "GBR", "FRA"],
  "indicator": "gdp",
  "threshold": 0.7,
  "method": "correlation"
}
```

**Response:**
```json
{
  "data": {
    "nodes": [
      {
        "id": "USA",
        "name": "United States",
        "centrality": 0.85,
        "degree": 4,
        "value": 26949.95
      }
    ],
    "edges": [
      {
        "source": "USA",
        "target": "CHN",
        "weight": 0.78,
        "type": "correlation"
      }
    ],
    "metrics": {
      "density": 0.67,
      "clustering_coefficient": 0.45,
      "average_path_length": 1.8
    }
  }
}
```

## Data Models

### Country Model
```typescript
interface Country {
  id: string;                    // ISO 3166-1 alpha-3 code
  name: string;                  // Full country name
  region: string;                // Geographic region
  subregion: string;             // Geographic subregion
  income_group: 'high' | 'upper_middle' | 'lower_middle' | 'low';
  population: number;            // Total population
  area: number;                  // Land area in square kilometers
  coordinates: {
    latitude: number;
    longitude: number;
  };
  economic_indicators: EconomicIndicator[];
  metadata: {
    last_updated: string;        // ISO 8601 timestamp
    data_quality: 'high' | 'medium' | 'low';
    completeness: number;        // 0-1 scale
  };
}
```

### Economic Indicator Model
```typescript
interface EconomicIndicator {
  name: string;                  // Indicator identifier
  display_name: string;          // Human-readable name
  description: string;           // Detailed description
  unit: string;                  // Unit of measurement
  category: 'economic' | 'monetary' | 'social' | 'environmental';
  frequency: 'daily' | 'monthly' | 'quarterly' | 'annual';
  sources: string[];             // Data sources
}
```

### Data Point Model
```typescript
interface DataPoint {
  date: string;                  // ISO 8601 date
  value: number;                 // Numeric value
  source: string;                // Data source
  quality_score?: number;        // 0-1 data quality score
  confidence_interval?: {
    lower: number;
    upper: number;
  };
}
```

## GraphQL Schema

### Schema Definition
```graphql
type Query {
  countries(
    limit: Int
    offset: Int
    region: String
    incomeGroup: IncomeGroup
    hasData: Boolean
  ): CountryConnection
  
  country(id: String!): Country
  
  indicators: [Indicator!]!
  
  indicatorData(
    indicator: String!
    countries: [String!]
    startDate: String
    endDate: String
    frequency: Frequency
  ): IndicatorData
  
  correlations(
    countries: [String!]!
    indicators: [String!]!
    startDate: String
    endDate: String
    method: CorrelationMethod
  ): CorrelationMatrix
  
  networkAnalysis(
    countries: [String!]!
    indicator: String!
    threshold: Float
    method: NetworkMethod
  ): NetworkGraph
}

type Country {
  id: String!
  name: String!
  region: String!
  subregion: String!
  incomeGroup: IncomeGroup!
  population: Int!
  area: Float!
  coordinates: Coordinates!
  economicIndicators: [EconomicIndicator!]!
  metadata: CountryMetadata!
}

type EconomicIndicator {
  name: String!
  value: Float!
  unit: String!
  date: String!
  source: String!
}

type Indicator {
  name: String!
  displayName: String!
  description: String!
  unit: String!
  category: IndicatorCategory!
  frequency: Frequency!
  sources: [String!]!
}

type CorrelationMatrix {
  matrix: JSON!
  significance: JSON!
  sampleSize: Int!
}

type NetworkGraph {
  nodes: [NetworkNode!]!
  edges: [NetworkEdge!]!
  metrics: NetworkMetrics!
}

enum IncomeGroup {
  HIGH
  UPPER_MIDDLE
  LOWER_MIDDLE
  LOW
}

enum IndicatorCategory {
  ECONOMIC
  MONETARY
  SOCIAL
  ENVIRONMENTAL
}

enum Frequency {
  DAILY
  MONTHLY
  QUARTERLY
  ANNUAL
}

enum CorrelationMethod {
  PEARSON
  SPEARMAN
  KENDALL
}

enum NetworkMethod {
  CORRELATION
  TRADE
  MIGRATION
}
```

### Example GraphQL Query
```graphql
query GetCountryData($countryId: String!) {
  country(id: $countryId) {
    id
    name
    region
    incomeGroup
    economicIndicators {
      name
      value
      unit
      date
    }
  }
  
  indicators {
    name
    displayName
    category
    frequency
  }
}
```

## Authentication

### API Key Authentication
```http
Authorization: Bearer YOUR_API_KEY
```

### OAuth 2.0
```http
Authorization: Bearer YOUR_ACCESS_TOKEN
```

### Rate Limiting Headers
```http
X-RateLimit-Limit: 1000
X-RateLimit-Remaining: 999
X-RateLimit-Reset: 1640995200
```

## Rate Limiting

### Limits
- **Free Tier**: 1,000 requests per hour
- **Pro Tier**: 10,000 requests per hour
- **Enterprise**: 100,000 requests per hour

### Headers
- `X-RateLimit-Limit`: Total requests allowed per hour
- `X-RateLimit-Remaining`: Requests remaining in current window
- `X-RateLimit-Reset`: Unix timestamp when limit resets

## Error Handling

### Error Response Format
```json
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Invalid country ID provided",
    "details": {
      "field": "country_id",
      "value": "INVALID",
      "constraint": "Must be valid ISO 3166-1 alpha-3 code"
    },
    "request_id": "req_123456789",
    "timestamp": "2024-01-15T10:30:00Z"
  }
}
```

### Error Codes
- `VALIDATION_ERROR`: Invalid request parameters
- `NOT_FOUND`: Resource not found
- `RATE_LIMIT_EXCEEDED`: Too many requests
- `UNAUTHORIZED`: Invalid or missing authentication
- `FORBIDDEN`: Insufficient permissions
- `INTERNAL_ERROR`: Server error
- `SERVICE_UNAVAILABLE`: Service temporarily unavailable

## Examples

### JavaScript/TypeScript
```typescript
// Using fetch API
const response = await fetch('https://api.econ-graph.com/v1/countries', {
  headers: {
    'Authorization': 'Bearer YOUR_API_KEY',
    'Content-Type': 'application/json'
  }
});

const data = await response.json();
console.log(data);

// Using axios
import axios from 'axios';

const api = axios.create({
  baseURL: 'https://api.econ-graph.com/v1',
  headers: {
    'Authorization': 'Bearer YOUR_API_KEY'
  }
});

const countries = await api.get('/countries?region=Europe&limit=50');
console.log(countries.data);
```

### Python
```python
import requests

headers = {
    'Authorization': 'Bearer YOUR_API_KEY',
    'Content-Type': 'application/json'
}

# Get countries
response = requests.get(
    'https://api.econ-graph.com/v1/countries',
    headers=headers,
    params={'region': 'Europe', 'limit': 50}
)

countries = response.json()
print(countries)

# Calculate correlations
correlation_data = {
    'countries': ['USA', 'CHN', 'DEU'],
    'indicators': ['gdp', 'inflation'],
    'start_date': '2020-01-01',
    'end_date': '2023-12-31'
}

response = requests.post(
    'https://api.econ-graph.com/v1/analysis/correlations',
    headers=headers,
    json=correlation_data
)

correlations = response.json()
print(correlations)
```

### GraphQL Example
```typescript
import { request } from 'graphql-request';

const query = `
  query GetCountries($region: String) {
    countries(region: $region) {
      data {
        id
        name
        region
        economicIndicators {
          name
          value
          unit
        }
      }
    }
  }
`;

const variables = { region: 'Europe' };
const data = await request('https://api.econ-graph.com/graphql', query, variables);
console.log(data);
```

## SDK Integration

### Official SDKs
- **JavaScript/TypeScript**: `npm install @econ-graph/sdk`
- **Python**: `pip install econ-graph-sdk`
- **R**: `install.packages("econGraph")`
- **Go**: `go get github.com/econ-graph/sdk-go`

### JavaScript SDK Example
```typescript
import { EconGraphClient } from '@econ-graph/sdk';

const client = new EconGraphClient({
  apiKey: 'YOUR_API_KEY',
  baseURL: 'https://api.econ-graph.com/v1'
});

// Get countries
const countries = await client.countries.list({
  region: 'Europe',
  limit: 50
});

// Get country details
const usa = await client.countries.get('USA');

// Calculate correlations
const correlations = await client.analysis.correlations({
  countries: ['USA', 'CHN', 'DEU'],
  indicators: ['gdp', 'inflation'],
  startDate: '2020-01-01',
  endDate: '2023-12-31'
});

// Generate network analysis
const network = await client.analysis.network({
  countries: ['USA', 'CHN', 'DEU', 'JPN'],
  indicator: 'gdp',
  threshold: 0.7
});
```

## Conclusion

The Global Analysis API provides comprehensive access to economic data and analysis tools through RESTful endpoints and GraphQL queries. The API is designed to be developer-friendly with clear documentation, multiple SDK options, and robust error handling. Rate limiting and authentication ensure secure and fair usage across all tiers.

The combination of REST and GraphQL endpoints allows developers to choose the most appropriate interface for their use case, while the comprehensive data models and analysis tools enable sophisticated economic analysis and visualization applications.
